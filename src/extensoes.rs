//! Ponte com o Lua.
//!
//! Este é o único módulo do projeto que importa `mlua`. Ele é responsável
//! por:
//!
//! 1. Varrer o diretório `extensions/` e carregar todo arquivo `.lua`
//!    encontrado (descoberta em tempo de execução, sem lista fixa).
//! 2. Expor à VM do Lua uma função de registro (`db.registrar`) que cada
//!    extensão chama, no seu próprio arquivo, para declarar qual prefixo de
//!    chave ela trata e quais operações (`add`, `get`) ela implementa.
//! 3. Expor à VM do Lua duas funções de consulta genéricas ao banco
//!    (`db.buscar` e `db.chaves`), que não sabem o que é CPF, data ou
//!    qualquer outro prefixo — elas só leem o `Armazenamento`.
//! 4. Despachar as chamadas de `ADD` e `GET` para a extensão correta,
//!    convertendo o retorno do Lua numa estrutura Rust (`Decisao`).
//!
//! Nenhum literal de prefixo (`"cpf"`, `"data"` etc.) aparece neste arquivo.
//! O motor em Rust não sabe quais extensões existem: ele só sabe que existe
//! *algum* conjunto de extensões, cada uma dizendo, em tempo de execução,
//! qual prefixo trata.
//!
//! # Protocolo de registro (ver também o README)
//!
//! Cada arquivo `.lua` em `extensions/`, ao ser carregado, deve chamar:
//!
//! ```lua
//! db.registrar("prefixo_", {
//!     add = function(chave, valor) ... end, -- opcional
//!     get = function(chave, valor) ... end, -- opcional
//! })
//! ```
//!
//! As funções `add`/`get` devem sempre retornar dois valores:
//! - `true, valor_resultante` em caso de sucesso (o valor a ser gravado, no
//!   `add`; o valor formatado a ser exibido, no `get`);
//! - `false, motivo` em caso de falha, onde `motivo` é uma string explicando
//!   o que deu errado.
//!
//! Dentro de `add`/`get` (ou de qualquer código chamado a partir delas), a
//! extensão pode chamar `db.buscar(chave)` (retorna o valor bruto gravado
//! sob `chave`, ou `nil` se a chave não existe) e `db.chaves()` (retorna uma
//! tabela-array com todas as chaves atualmente gravadas). Essas duas funções
//! sempre refletem o estado atual do banco no instante da chamada — elas não
//! recebem uma cópia.

use mlua::{Function, Lua, Table};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use crate::armazenamento::Armazenamento;

/// O que a ponte decidiu fazer com um `ADD` ou `GET`: ou nenhuma extensão
/// trata o prefixo da chave (o motor deve seguir com o comportamento
/// padrão, isto é, passar o valor direto), ou uma extensão trata, e o
/// resultado dela (sucesso com o valor resultante, ou falha com o motivo)
/// vem dentro de `ComExtensao`.
pub enum Decisao {
    SemExtensao,
    ComExtensao(Result<String, String>),
}

/// Uma extensão registrada: o prefixo que ela trata e as funções Lua que
/// implementam `ADD` e/ou `GET` (qualquer uma das duas pode estar ausente).
struct Extensao {
    prefixo: String,
    add: Option<Function>,
    get: Option<Function>,
}

/// A ponte em si. Mantém a VM do Lua viva (as `Function`s guardadas em
/// `extensoes` dependem dela existir) e a lista de extensões descobertas no
/// boot.
pub struct Ponte {
    _lua: Lua,
    extensoes: Vec<Extensao>,
}

impl Ponte {
    /// Varre `diretorio` em busca de arquivos `.lua`, carrega cada um deles
    /// (o que dispara as chamadas a `db.registrar` dentro de cada arquivo) e
    /// devolve a ponte pronta para uso.
    ///
    /// `armazenamento` é o mesmo handle usado pelo resto do programa: as
    /// funções `db.buscar`/`db.chaves` expostas ao Lua leem diretamente dele,
    /// nunca de uma cópia.
    pub fn carregar(diretorio: &str, armazenamento: Armazenamento) -> Result<Self, String> {
        let lua = Lua::new();

        // Registro compartilhado: a função `db.registrar` (chamada de
        // dentro do Lua, uma vez por extensão) empilha aqui. Depois que
        // todos os arquivos forem carregados, isso é consumido e vira o
        // `Vec<Extensao>` definitivo da ponte.
        let registro: Rc<RefCell<Vec<Extensao>>> = Rc::new(RefCell::new(Vec::new()));

        let tabela_db = lua.create_table().map_err(|e| e.to_string())?;

        registrar_funcao_registro(&lua, &tabela_db, Rc::clone(&registro))?;
        registrar_funcao_buscar(&lua, &tabela_db, armazenamento.clone())?;
        registrar_funcao_chaves(&lua, &tabela_db, armazenamento.clone())?;

        lua.globals()
            .set("db", tabela_db)
            .map_err(|e| e.to_string())?;

        for caminho in listar_arquivos_lua(diretorio)? {
            let fonte = fs::read_to_string(&caminho)
                .map_err(|e| format!("não foi possível ler '{}': {}", caminho.display(), e))?;

            lua.load(&fonte)
                .exec()
                .map_err(|e| format!("erro ao carregar '{}': {}", caminho.display(), e))?;
        }

        // Neste ponto, todo `Rc::clone(&registro)` que existia foi só o que
        // demos para a closure de `db.registrar` — e essa closure já
        // terminou de rodar (o carregamento dos arquivos já acabou). Só
        // sobra este `registro`, então o unwrap é seguro.
        let extensoes = Rc::try_unwrap(registro)
            .map_err(|_| "referência pendente ao registro de extensões".to_string())?
            .into_inner();

        Ok(Ponte {
            _lua: lua,
            extensoes,
        })
    }

    /// Encontra a extensão cujo prefixo casa com a chave. Quando mais de um
    /// prefixo casa (situação que não deveria acontecer nas extensões deste
    /// projeto, mas que o design não proíbe), vence o prefixo mais longo —
    /// ou seja, o mais específico.
    fn encontrar(&self, chave: &str) -> Option<&Extensao> {
        self.extensoes
            .iter()
            .filter(|extensao| chave.starts_with(&extensao.prefixo))
            .max_by_key(|extensao| extensao.prefixo.len())
    }

    /// Processa um `ADD`: se alguma extensão trata o prefixo da chave *e*
    /// implementa `add`, chama a função Lua e devolve o resultado. Caso
    /// contrário, devolve `SemExtensao` (o motor deve gravar o valor como
    /// veio, sem validação).
    pub fn validar_add(&self, chave: &str, valor: &str) -> Decisao {
        let Some(extensao) = self.encontrar(chave) else {
            return Decisao::SemExtensao;
        };
        let Some(funcao) = &extensao.add else {
            return Decisao::SemExtensao;
        };
        Decisao::ComExtensao(chamar(funcao, chave, valor))
    }

    /// Processa um `GET`: se alguma extensão trata o prefixo da chave *e*
    /// implementa `get`, chama a função Lua (passando o valor bruto
    /// gravado) e devolve o resultado formatado. Caso contrário, devolve
    /// `SemExtensao` (o motor deve exibir o valor bruto).
    pub fn formatar_get(&self, chave: &str, valor_bruto: &str) -> Decisao {
        let Some(extensao) = self.encontrar(chave) else {
            return Decisao::SemExtensao;
        };
        let Some(funcao) = &extensao.get else {
            return Decisao::SemExtensao;
        };
        Decisao::ComExtensao(chamar(funcao, chave, valor_bruto))
    }
}

/// Chama uma função Lua de `add`/`get` seguindo o protocolo
/// `(ok: bool, valor_ou_motivo: string)` e converte o retorno (ou um erro
/// levantado dentro do Lua, incluindo `error()`) para `Result<String, String>`.
/// É aqui que um erro do Lua vira, definitivamente, uma `String` de motivo —
/// o chamador (em `main.rs`) só precisa prefixar isso com `"ERRO: "`.
fn chamar(funcao: &Function, chave: &str, valor: &str) -> Result<String, String> {
    let resultado: mlua::Result<(bool, String)> =
        funcao.call((chave.to_string(), valor.to_string()));

    match resultado {
        Ok((true, valor_final)) => Ok(valor_final),
        Ok((false, motivo)) => Err(motivo),
        // Erro de execução do Lua: erro de sintaxe não chega aqui (isso
        // falha no carregamento do arquivo, em `carregar`), mas um
        // `error(...)` chamado dentro da extensão, um acesso a variável
        // inexistente, ou um retorno no formato errado, caem aqui.
        Err(erro_lua) => Err(format!("erro na extensão: {}", erro_lua)),
    }
}

/// Lista, em ordem determinística, os caminhos de todos os arquivos `.lua`
/// diretamente dentro de `diretorio`.
fn listar_arquivos_lua(diretorio: &str) -> Result<Vec<PathBuf>, String> {
    let entradas = fs::read_dir(diretorio)
        .map_err(|e| format!("não foi possível abrir o diretório '{}': {}", diretorio, e))?;

    let mut arquivos: Vec<PathBuf> = entradas
        .filter_map(|entrada| entrada.ok())
        .map(|entrada| entrada.path())
        .filter(|caminho| {
            caminho
                .extension()
                .map(|ext| ext == "lua")
                .unwrap_or(false)
        })
        .collect();

    // Ordem determinística de carregamento (não afeta a corretude do
    // protocolo, já que cada extensão só depende do que ELA registra, mas
    // torna o comportamento do programa reprodutível e os logs de erro
    // previsíveis).
    arquivos.sort();

    Ok(arquivos)
}

/// Expõe `db.registrar(prefixo, tabela)` ao Lua.
fn registrar_funcao_registro(
    lua: &Lua,
    tabela_db: &Table,
    registro: Rc<RefCell<Vec<Extensao>>>,
) -> Result<(), String> {
    let funcao = lua
        .create_function(move |_, (prefixo, tabela): (String, Table)| {
            // Campos ausentes (nil) viram None; campos presentes mas de tipo
            // errado também viram None nesta implementação — uma extensão
            // malformada simplesmente não trata a operação correspondente,
            // em vez de derrubar o carregamento do banco inteiro.
            let add: Option<Function> = tabela.get("add").unwrap_or(None);
            let get: Option<Function> = tabela.get("get").unwrap_or(None);
            registro.borrow_mut().push(Extensao { prefixo, add, get });
            Ok(())
        })
        .map_err(|e| e.to_string())?;

    tabela_db
        .set("registrar", funcao)
        .map_err(|e| e.to_string())
}

/// Expõe `db.buscar(chave) -> valor:string|nil` ao Lua. Genérica: só chama
/// `Armazenamento::buscar`, sem saber o que é uma chave "de CPF" ou "de
/// data" ou qualquer outra coisa.
fn registrar_funcao_buscar(
    lua: &Lua,
    tabela_db: &Table,
    armazenamento: Armazenamento,
) -> Result<(), String> {
    let funcao = lua
        .create_function(move |_, chave: String| Ok(armazenamento.buscar(&chave)))
        .map_err(|e| e.to_string())?;

    tabela_db.set("buscar", funcao).map_err(|e| e.to_string())
}

/// Expõe `db.chaves() -> {chave1, chave2, ...}` ao Lua. Também genérica.
fn registrar_funcao_chaves(
    lua: &Lua,
    tabela_db: &Table,
    armazenamento: Armazenamento,
) -> Result<(), String> {
    let funcao = lua
        .create_function(move |lua, ()| {
            let tabela = lua.create_table()?;
            for (indice, chave) in armazenamento.chaves().into_iter().enumerate() {
                tabela.set(indice + 1, chave)?;
            }
            Ok(tabela)
        })
        .map_err(|e| e.to_string())?;

    tabela_db.set("chaves", funcao).map_err(|e| e.to_string())
}

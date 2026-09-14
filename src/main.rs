//! `banco-memoria` — banco de dados chave-valor em memória com extensões em
//! Lua.
//!
//! `main.rs` não contém nenhuma lógica própria: ele só monta as peças
//! (armazenamento + ponte com o Lua) e entrega o controle ao laço de
//! leitura. Ver `README.md` para a arquitetura completa.

mod armazenamento;
mod comando;
mod extensoes;
mod repl;

use armazenamento::Armazenamento;
use extensoes::Ponte;

/// Diretório varrido em busca de extensões `.lua`, relativo ao diretório de
/// onde o executável é chamado.
const DIRETORIO_EXTENSOES: &str = "extensions";

fn main() {
    let armazenamento = Armazenamento::novo();

    let ponte = match Ponte::carregar(DIRETORIO_EXTENSOES, armazenamento.clone()) {
        Ok(ponte) => ponte,
        Err(motivo) => {
            eprintln!(
                "Não foi possível carregar as extensões em '{}': {}",
                DIRETORIO_EXTENSOES, motivo
            );
            std::process::exit(1);
        }
    };

    repl::executar(armazenamento, ponte);
}

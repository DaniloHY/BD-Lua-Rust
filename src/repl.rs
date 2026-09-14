//! Leitura da entrada.
//!
//! Este módulo é o laço interativo: imprime o prompt (`> `), lê uma linha
//! (do teclado ou de um pipe, não faz diferença aqui), pede para o módulo
//! `comando` interpretá-la, executa o efeito correspondente usando
//! `Armazenamento` e `Ponte`, e escreve a resposta.
//!
//! Este módulo NÃO importa `mlua` diretamente — ele só chama métodos
//! públicos de `extensoes::Ponte`. Quem sabe que existe uma VM de Lua por
//! trás é só o módulo `extensoes`.

use std::io::{self, BufRead, Write};

use crate::armazenamento::Armazenamento;
use crate::comando::{self, Comando};
use crate::extensoes::{Decisao, Ponte};

/// Roda o laço principal até receber `EXIT` ou até a entrada acabar (fim de
/// um pipe), o que, pela especificação, tem o mesmo efeito de um `EXIT`.
pub fn executar(armazenamento: Armazenamento, ponte: Ponte) {
    let entrada = io::stdin();
    let mut saida = io::stdout();
    let mut linha = String::new();

    loop {
        // O prompt é sempre impresso, tanto no modo interativo quanto
        // quando a entrada vem de um pipe — é assim que o exemplo do
        // enunciado mostra a transcrição, e não há razão para diferenciar
        // os dois modos (ver README, seção de decisões de projeto).
        print!("> ");
        if saida.flush().is_err() {
            // Se nem a saída padrão está disponível, não há mais o que
            // fazer além de encerrar.
            break;
        }

        linha.clear();
        match entrada.lock().read_line(&mut linha) {
            Ok(0) => {
                // Fim da entrada (pipe fechado / Ctrl+D): equivale a EXIT.
                println!();
                break;
            }
            Ok(_) => {}
            Err(erro) => {
                println!("ERRO: falha ao ler a entrada ({})", erro);
                continue;
            }
        }

        match comando::interpretar(&linha) {
            Ok(None) => {
                // Linha em branco: nada a fazer, o laço já volta ao prompt.
            }
            Ok(Some(Comando::Exit)) => break,
            Ok(Some(Comando::Add { chave, valor })) => {
                match processar_add(&armazenamento, &ponte, &chave, &valor) {
                    Ok(()) => println!("OK"),
                    Err(motivo) => println!("ERRO: {}", motivo),
                }
            }
            Ok(Some(Comando::Get { chave })) => {
                match processar_get(&armazenamento, &ponte, &chave) {
                    Ok(valor) => println!("{}", valor),
                    Err(motivo) => println!("ERRO: {}", motivo),
                }
            }
            Err(motivo) => println!("ERRO: {}", motivo),
        }
    }
}

/// Executa um `ADD`: pergunta à ponte se alguma extensão trata esta chave;
/// se sim, usa o resultado dela (que pode ser uma versão normalizada do
/// valor, ou uma falha); se não, grava o valor exatamente como recebido.
///
/// Importante: a chamada à ponte (que pode, por baixo dos panos, disparar
/// consultas de leitura ao banco a partir do Lua) acontece ANTES de
/// qualquer gravação. Só depois que a validação termina — com sucesso — é
/// que `armazenamento.gravar` é chamado. Isso é o que evita o problema
/// descrito no enunciado (o `ADD` já estar "no meio" de uma operação sobre o
/// banco no instante em que a extensão consulta): aqui, no instante da
/// consulta, o `ADD` ainda não tocou o armazenamento para escrita nenhuma
/// vez, então não há nenhum empréstimo mutável pendente para conflitar.
fn processar_add(
    armazenamento: &Armazenamento,
    ponte: &Ponte,
    chave: &str,
    valor: &str,
) -> Result<(), String> {
    match ponte.validar_add(chave, valor) {
        Decisao::SemExtensao => {
            armazenamento.gravar(chave, valor);
            Ok(())
        }
        Decisao::ComExtensao(Ok(valor_final)) => {
            armazenamento.gravar(chave, &valor_final);
            Ok(())
        }
        Decisao::ComExtensao(Err(motivo)) => Err(motivo),
    }
}

/// Executa um `GET`: busca o valor bruto; se a chave não existe, erro. Caso
/// exista, pergunta à ponte se alguma extensão formata esta chave; se não
/// houver extensão (ou ela não implementar `get`), devolve o valor bruto.
fn processar_get(armazenamento: &Armazenamento, ponte: &Ponte, chave: &str) -> Result<String, String> {
    let valor_bruto = armazenamento
        .buscar(chave)
        .ok_or_else(|| "chave inexistente".to_string())?;

    match ponte.formatar_get(chave, &valor_bruto) {
        Decisao::SemExtensao => Ok(valor_bruto),
        Decisao::ComExtensao(Ok(valor_formatado)) => Ok(valor_formatado),
        Decisao::ComExtensao(Err(motivo)) => Err(motivo),
    }
}

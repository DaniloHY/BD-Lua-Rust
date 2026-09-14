//! Interpretação de comandos.
//!
//! Responsabilidade única deste módulo: transformar uma linha de texto crua
//! (como veio do teclado ou de um pipe) em um `Comando` estruturado, ou em um
//! erro de sintaxe. Este módulo não conhece armazenamento nem extensões —
//! ele só entende a gramática dos três comandos da interface.

/// Um comando já reconhecido a partir de uma linha de texto.
#[derive(Debug, PartialEq, Eq)]
pub enum Comando {
    Add { chave: String, valor: String },
    Get { chave: String },
    Exit,
}

/// Interpreta uma linha digitada pelo usuário (ou lida de um pipe).
///
/// Retorna:
/// - `Ok(Some(comando))` quando a linha é um comando válido;
/// - `Ok(None)` quando a linha está em branco (não é erro, só não há nada a
///   fazer: o prompt volta em silêncio);
/// - `Err(motivo)` quando a linha não corresponde a nenhum comando válido.
///   O `motivo` é só o texto explicativo, sem o prefixo `ERRO: ` — quem
///   consome o retorno decide como exibir o erro.
///
/// Regras de sintaxe (ver README para a justificativa de cada uma):
/// - O nome do comando é sempre o primeiro "token" da linha (até o primeiro
///   espaço), e é sensível a maiúsculas: `add` não é `ADD`.
/// - Em `ADD chave valor`, o valor é tudo que vem depois do primeiro espaço
///   que segue a chave — por isso o valor pode conter espaços, mas a chave
///   nunca pode. Se não houver nada depois da chave (nem um espaço), o valor
///   é a string vazia: isso NÃO é um erro de sintaxe. Uma extensão pode
///   rejeitar valor vazio por conta própria (é o caso do CPF e da Data), mas
///   isso é uma regra de validação da extensão, não da gramática do comando.
/// - `ADD` sem nenhuma chave, ou `GET` sem nenhuma chave, é comando
///   incompleto.
pub fn interpretar(linha_bruta: &str) -> Result<Option<Comando>, String> {
    // Remove só o fim de linha (\n / \r\n) que read_line deixa. O restante
    // dos espaços é preservado deliberadamente: espaços no meio ou no fim de
    // um valor fazem parte do valor.
    let linha = linha_bruta.trim_end_matches(['\n', '\r']);

    // Espaços de indentação no começo da linha não fazem parte de nenhum
    // comando válido, então podem ser descartados com segurança.
    let linha = linha.trim_start();

    if linha.trim().is_empty() {
        return Ok(None);
    }

    let (nome_comando, resto) = match linha.split_once(' ') {
        Some((comando, resto)) => (comando, Some(resto)),
        None => (linha, None),
    };

    match nome_comando {
        "EXIT" => Ok(Some(Comando::Exit)),

        "GET" => {
            let chave = resto.unwrap_or("").trim();
            if chave.is_empty() {
                Err("comando incompleto".to_string())
            } else {
                Ok(Some(Comando::Get {
                    chave: chave.to_string(),
                }))
            }
        }

        "ADD" => {
            let resto = resto.ok_or_else(|| "comando incompleto".to_string())?;

            let (chave, valor) = match resto.split_once(' ') {
                Some((chave, valor)) => (chave, valor.to_string()),
                // Só a chave, sem espaço depois: valor vazio (não é erro de
                // sintaxe, ver comentário da função).
                None => (resto, String::new()),
            };

            if chave.is_empty() {
                Err("comando incompleto".to_string())
            } else {
                Ok(Some(Comando::Add {
                    chave: chave.to_string(),
                    valor,
                }))
            }
        }

        _ => Err("comando desconhecido".to_string()),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn linha_em_branco_nao_e_erro() {
        assert_eq!(interpretar(""), Ok(None));
        assert_eq!(interpretar("   "), Ok(None));
        assert_eq!(interpretar("\n"), Ok(None));
    }

    #[test]
    fn exit_simples() {
        assert_eq!(interpretar("EXIT"), Ok(Some(Comando::Exit)));
    }

    #[test]
    fn get_com_chave() {
        assert_eq!(
            interpretar("GET cpf_a"),
            Ok(Some(Comando::Get {
                chave: "cpf_a".to_string()
            }))
        );
    }

    #[test]
    fn get_sem_chave_e_incompleto() {
        assert_eq!(interpretar("GET"), Err("comando incompleto".to_string()));
    }

    #[test]
    fn add_com_valor_com_espacos() {
        assert_eq!(
            interpretar("ADD nome_zezinho Jose da Silva"),
            Ok(Some(Comando::Add {
                chave: "nome_zezinho".to_string(),
                valor: "Jose da Silva".to_string()
            }))
        );
    }

    #[test]
    fn add_so_com_chave_tem_valor_vazio() {
        assert_eq!(
            interpretar("ADD cpf_k"),
            Ok(Some(Comando::Add {
                chave: "cpf_k".to_string(),
                valor: String::new()
            }))
        );
    }

    #[test]
    fn add_sem_nada_e_incompleto() {
        assert_eq!(interpretar("ADD"), Err("comando incompleto".to_string()));
    }

    #[test]
    fn comando_desconhecido() {
        assert_eq!(
            interpretar("DELETE cpf_a"),
            Err("comando desconhecido".to_string())
        );
        assert_eq!(
            interpretar("add cpf_a 123"),
            Err("comando desconhecido".to_string())
        );
    }
}

//! Armazenamento chave-valor em memória.
//!
//! Este módulo é deliberadamente "burro": ele só sabe gravar, buscar e
//! listar chaves. Ele não importa `mlua`, não sabe que extensões existem, e
//! não sabe o que é um CPF ou uma data — essa regra é o que garante que a
//! separação de responsabilidades pedida no enunciado seja real, e não só
//! nominal.
//!
//! Os dados vivem atrás de `Rc<RefCell<..>>` em vez de, por exemplo,
//! `Arc<Mutex<..>>`, porque o programa inteiro é de uma única thread (um
//! laço de leitura de comandos, sem concorrência real). `RefCell` também é a
//! peça central de como a consulta ao banco a partir do Lua foi resolvida
//! sem recursão problemática — ver o módulo `extensoes` e o README para os
//! detalhes.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Um handle barato de clonar (clona só o `Rc`, não os dados) para o
/// armazenamento em memória. Passar este tipo por valor para outras partes
/// do programa (como a ponte com o Lua) não copia o banco: todos os clones
/// enxergam os mesmos dados.
#[derive(Clone)]
pub struct Armazenamento {
    dados: Rc<RefCell<HashMap<String, String>>>,
}

impl Armazenamento {
    /// Cria um banco vazio.
    pub fn novo() -> Self {
        Armazenamento {
            dados: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    /// Grava (ou sobrescreve) o valor associado a uma chave.
    ///
    /// Toma emprestado o `RefCell` de forma mutável só pelo tempo desta
    /// chamada: o empréstimo é solto assim que a função retorna.
    pub fn gravar(&self, chave: &str, valor: &str) {
        self.dados
            .borrow_mut()
            .insert(chave.to_string(), valor.to_string());
    }

    /// Busca o valor bruto (como foi gravado, sem nenhuma formatação de
    /// extensão) associado a uma chave. `None` se a chave não existe.
    ///
    /// O empréstimo imutável do `RefCell` dura só o tempo desta chamada.
    /// Isso é o que permite que esta função seja chamada livremente durante
    /// a validação de um `ADD` em andamento (que só toma o empréstimo
    /// mutável no final, ao gravar) sem entrar em conflito com ela.
    pub fn buscar(&self, chave: &str) -> Option<String> {
        self.dados.borrow().get(chave).cloned()
    }

    /// Lista todas as chaves atualmente gravadas no banco. Usada por
    /// extensões que precisam varrer o banco inteiro (como o validador de
    /// CPF, para checar unicidade).
    pub fn chaves(&self) -> Vec<String> {
        self.dados.borrow().keys().cloned().collect()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn grava_e_busca() {
        let banco = Armazenamento::novo();
        banco.gravar("a", "1");
        assert_eq!(banco.buscar("a"), Some("1".to_string()));
        assert_eq!(banco.buscar("b"), None);
    }

    #[test]
    fn sobrescreve() {
        let banco = Armazenamento::novo();
        banco.gravar("a", "1");
        banco.gravar("a", "2");
        assert_eq!(banco.buscar("a"), Some("2".to_string()));
    }

    #[test]
    fn clones_compartilham_os_mesmos_dados() {
        let banco = Armazenamento::novo();
        let outro_handle = banco.clone();
        banco.gravar("a", "1");
        assert_eq!(outro_handle.buscar("a"), Some("1".to_string()));
    }

    #[test]
    fn lista_chaves() {
        let banco = Armazenamento::novo();
        banco.gravar("a", "1");
        banco.gravar("b", "2");
        let mut chaves = banco.chaves();
        chaves.sort();
        assert_eq!(chaves, vec!["a".to_string(), "b".to_string()]);
    }
}

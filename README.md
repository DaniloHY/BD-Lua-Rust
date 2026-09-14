# banco-memoria

Banco de dados chave-valor em memória, em Rust, com um sistema de extensões
escritas em Lua.

## Integrantes

> **Preencher antes da entrega** com o nome completo de todos os
> integrantes do grupo — quem não estiver listado aqui não recebe nota por
> este trabalho, mesmo que apareça no histórico de commits.

- [Nome completo do integrante 1]
- [Nome completo do integrante 2]
- [Nome completo do integrante 3]

---

## Sumário

1. [Como compilar e executar](#como-compilar-e-executar)
2. [Visão geral da arquitetura](#visão-geral-da-arquitetura)
3. [Módulos do projeto](#módulos-do-projeto)
4. [O protocolo de registro de extensões](#o-protocolo-de-registro-de-extensões)
5. [Estruturas de retorno e como um erro do Lua vira `ERRO:`](#estruturas-de-retorno-e-como-um-erro-do-lua-vira-erro)
6. [Como acrescentar uma extensão nova](#como-acrescentar-uma-extensão-nova)
7. [A consulta ao banco a partir da extensão](#a-consulta-ao-banco-a-partir-da-extensão)
8. [Extensões obrigatórias](#extensões-obrigatórias)
9. [Extensão proposta pelo grupo: referência entre chaves](#extensão-proposta-pelo-grupo-referência-entre-chaves)
10. [Decisões de projeto](#decisões-de-projeto)

---

## Como compilar e executar

Pré-requisitos numa máquina limpa: só o **Rust** (`rustc`/`cargo`, via
[rustup](https://rustup.rs/)) e um **compilador C** (`gcc` ou `clang`), que
normalmente já vem junto do toolchain de build usado pelo Rust em Linux e
macOS. Não é preciso instalar Lua separadamente: a dependência `mlua` está
configurada com a feature `vendored`, que compila o Lua 5.4 junto do
projeto.

```bash
# na raiz do repositório
cargo build --release

# executa (o binário procura o diretório extensions/ relativo ao diretório
# de onde ele é chamado, então rode a partir da raiz do repositório, ou
# copie a pasta extensions/ para perto do binário)
./target/release/banco-memoria
```

Modo interativo:

```
$ ./target/release/banco-memoria
> ADD cpf_zezinho 12345678909
OK
> GET cpf_zezinho
123.456.789-09
> EXIT
$
```

Rodando um roteiro de comandos de uma vez (o fim da entrada equivale a um
`EXIT`):

```bash
grep -v '^#' casos_teste.txt | sed 's/ *->.*//' | ./target/release/banco-memoria
```

(o `casos_teste.txt` tem comentários e a coluna "resultado esperado"
separados por `->`; o `grep`/`sed` acima é só uma forma rápida de extrair
somente os comandos para alimentar o programa — não faz parte do projeto).

Para rodar os testes automatizados do parser de comandos e do armazenamento
(unitários, em Rust; não testam as extensões Lua, que são validadas via
`casos_teste.txt` e `casos_teste_referencia.txt` rodando o binário de
verdade):

```bash
cargo test
```

---

## Visão geral da arquitetura

```
main.rs
  ├─ armazenamento.rs   (HashMap chave-valor; não conhece Lua)
  ├─ extensoes.rs        (ponte com o Lua; único módulo que importa mlua)
  └─ repl.rs             (laço de leitura; usa os dois módulos acima)
       └─ comando.rs      (interpretação de uma linha de texto)
```

O fluxo de um `ADD chave valor`:

1. `repl` lê a linha e pede para `comando::interpretar` transformá-la num
   `Comando::Add { chave, valor }` (ou reportar um erro de sintaxe).
2. `repl` pergunta para `extensoes::Ponte::validar_add(chave, valor)` se
   alguma extensão trata o prefixo desta chave.
   - Se nenhuma tratar, o valor é gravado exatamente como veio.
   - Se alguma tratar, a ponte chama a função Lua `add` registrada por ela,
     que pode (a) consultar o banco através de `db.buscar`/`db.chaves`, (b)
     devolver sucesso com um valor (possivelmente transformado) a gravar, ou
     (c) devolver falha com um motivo.
3. `repl` grava o valor (bruto ou o devolvido pela extensão) no
   `armazenamento`, ou imprime `ERRO: <motivo>`.

O fluxo de `GET chave` é análogo, trocando `validar_add` por
`formatar_get` e "valor a gravar" por "valor a exibir".

---

## Módulos do projeto

| Módulo | Responsabilidade | Depende de |
|---|---|---|
| `comando.rs` | Interpretar uma linha de texto como `ADD`/`GET`/`EXIT`, ou erro de sintaxe. | Nada (só `std`). |
| `armazenamento.rs` | Guardar e recuperar pares chave-valor em memória (`Rc<RefCell<HashMap<..>>>`). | Nada (só `std`). |
| `extensoes.rs` | Carregar `extensions/*.lua`, expor `db.registrar`/`db.buscar`/`db.chaves` ao Lua, despachar `ADD`/`GET` para a extensão correta. **Único módulo que faz `use mlua::...`.** | `armazenamento.rs` (para implementar `db.buscar`/`db.chaves`) e `mlua`. |
| `repl.rs` | Laço do prompt: lê uma linha, chama `comando::interpretar`, executa o efeito usando `armazenamento` e `extensoes::Ponte`, escreve a resposta. | `comando.rs`, `armazenamento.rs`, `extensoes.rs` (só a API pública — não importa `mlua`). |
| `main.rs` | Monta `Armazenamento` + `Ponte` e chama `repl::executar`. Sem lógica própria. | Todos os outros. |

A regra "o crate do Lua só pode ser importado em um módulo" é satisfeita
porque `repl.rs` (e `main.rs`) só chamam métodos públicos de
`extensoes::Ponte` (`validar_add`, `formatar_get`) e o tipo público
`Decisao` — nenhum deles precisa saber que existe uma `struct Lua` por trás.

---

## O protocolo de registro de extensões

Uma extensão é um arquivo `.lua` dentro de `extensions/`. Ao ser carregado
(o motor executa o arquivo do início ao fim, como um script Lua comum), ele
deve chamar a função global `db.registrar`, que o motor já deixou disponível
antes de carregar qualquer extensão:

```lua
db.registrar("meu_prefixo_", {
    add = function(chave, valor)
        -- roda quando alguém faz ADD numa chave que começa com
        -- "meu_prefixo_". Deve devolver:
        --   true, valor_para_gravar     -- sucesso
        --   false, "motivo do erro"     -- falha
    end,

    get = function(chave, valor)
        -- roda quando alguém faz GET numa chave que começa com
        -- "meu_prefixo_". `valor` é o valor BRUTO gravado (o que o `add`
        -- devolveu, ou o valor original se não houver `add`). Deve
        -- devolver:
        --   true, valor_formatado       -- sucesso
        --   false, "motivo do erro"     -- falha (raro; normalmente só faz
        --                                  sentido se o dado gravado está,
        --                                  de alguma forma, corrompido)
    end,
})
```

Pontos importantes do protocolo:

- **`add` e `get` são independentes e opcionais.** Uma extensão pode
  implementar só um dos dois. Se `add` estiver ausente, `ADD` numa chave com
  aquele prefixo grava o valor sem nenhuma validação. Se `get` estiver
  ausente, `GET` devolve o valor bruto, sem formatação. (O enunciado deixa
  isso em aberto — "quais operações implementa" — e o suporte a "só uma das
  duas" é o motivo de `add`/`get` serem chaves separadas na tabela, em vez
  de, por exemplo, dois `db.registrar` diferentes.)
- **O prefixo é uma string literal comparada com `chave:find(prefixo) ==
  1`** (na prática, `chave:sub(1, #prefixo) == prefixo` do lado do Rust, via
  `str::starts_with`). Se duas extensões tiverem prefixos onde um é prefixo
  do outro (por exemplo `"a_"` e `"a_b_"`), o motor usa o **mais longo**
  (mais específico) que casar com a chave.
- **O retorno é sempre um par `(bool, string)`.** Isso vale tanto para
  sucesso quanto para falha — a extensão sempre explica o resultado, nunca
  só sinaliza sucesso/falha sem dizer o valor ou o motivo. Ver a próxima
  seção para como isso vira uma estrutura Rust.
- O nome da função de registro (`db.registrar`) e das funções de consulta
  (`db.buscar`, `db.chaves`, explicadas [mais abaixo](#a-consulta-ao-banco-a-partir-da-extensão))
  são a única API que uma extensão precisa conhecer. Nenhuma delas é
  específica de CPF, data, ou qualquer outra extensão — são as mesmas três
  funções para qualquer extensão que alguém escrever.

---

## Estruturas de retorno e como um erro do Lua vira `ERRO:`

No lado do Rust (`extensoes.rs`), o resultado de perguntar à ponte "alguma
extensão trata esta chave?" é o enum:

```rust
pub enum Decisao {
    SemExtensao,
    ComExtensao(Result<String, String>),
}
```

- `SemExtensao`: nenhum prefixo registrado casa com a chave (ou casa, mas a
  extensão não implementa a operação pedida). Quem chamou deve seguir com o
  comportamento padrão (gravar/exibir o valor cru).
- `ComExtensao(Ok(valor))`: a extensão tratou a chave e teve sucesso;
  `valor` é o valor a gravar (`ADD`) ou a exibir (`GET`).
- `ComExtensao(Err(motivo))`: a extensão tratou a chave e rejeitou a
  operação; `motivo` é a explicação, que `repl.rs` imprime como
  `ERRO: <motivo>`.

Ou seja: a "estrutura de sucesso" e a "estrutura de erro" pedidas pelo
enunciado são, respectivamente, as variantes `Ok(String)` e `Err(String)` do
próprio `Result` da biblioteca padrão, especializado para este uso através
do `Decisao`. Não criamos um tipo de erro mais elaborado (com código,
categoria, etc.) porque a interface do banco só precisa de uma mensagem de
texto — um `enum` de erros "engessaria" as extensões sem necessidade, já que
o motor não conhece as extensões e não teria como reagir de forma diferente
a cada categoria de erro.

Como um erro chega até a tela: dentro de `extensoes::chamar` (a função que
efetivamente invoca a função Lua de `add`/`get`), a chamada é feita como:

```rust
let resultado: mlua::Result<(bool, String)> = funcao.call((chave, valor));
```

Há duas formas de "erro" possíveis aqui, e as duas viram uma `String` antes
de sair desta função:

1. **A extensão devolve `false, "motivo"` normalmente** (um retorno Lua
   válido, só que sinalizando falha). Isso cai no braço
   `Ok((false, motivo))` do `match`, e vira `Err(motivo)`.
2. **A chamada Lua falha de verdade** — por exemplo, a extensão chama
   `error("algo")`, tenta indexar uma variável inexistente, ou devolve algo
   que não é um par `(bool, string)` (como só `true`, sem o segundo valor).
   O `mlua` transforma isso num `mlua::Error`, que cai no braço
   `Err(erro_lua)` do `match` e vira `Err(format!("erro na extensão: {}",
   erro_lua))`.

Nos dois casos, o resultado final é um `Result<String, String>` (dentro de
`Decisao::ComExtensao`), e `repl.rs` só precisa saber imprimir
`ERRO: {motivo}` quando encontra um `Err` — ele nunca precisa saber se o
erro veio de um `return false, "..."` deliberado ou de uma falha de
execução do Lua. Isso também é o motivo pelo qual **nenhuma entrada derruba
o processo**: um erro de Lua (sintaxe é pego no carregamento; erro de
execução é pego aqui) nunca propaga como um `panic!` do Rust, sempre vira
uma `String` de erro comum.

---

## Como acrescentar uma extensão nova

Passo a passo para quem nunca viu este projeto (é exatamente isso que será
testado na correção, com uma extensão desconhecida):

1. Crie um arquivo `.lua` dentro do diretório `extensions/` (o nome do
   arquivo é livre; só a extensão `.lua` importa).
2. Escreva uma ou duas funções — uma para `ADD`, outra para `GET` — cada
   uma recebendo `(chave, valor)` e devolvendo `true, valor_resultante` ou
   `false, "motivo"`. Nenhuma das duas é obrigatória, mas pelo menos uma
   deve existir para a extensão fazer algo.
3. Se a validação do `ADD` precisar saber o que já está gravado no banco
   (não só o valor recebido), use `db.buscar("uma_chave")` (devolve o valor
   gravado sob aquela chave, ou `nil`) e/ou `db.chaves()` (devolve uma
   tabela-array com todas as chaves gravadas no momento).
4. No fim do arquivo, chame:
   ```lua
   db.registrar("seu_prefixo_", { add = sua_funcao_add, get = sua_funcao_get })
   ```
5. Coloque o arquivo em `extensions/` e rode o binário — **sem recompilar
   nada**. No próximo boot, o motor varre o diretório de novo e encontra o
   arquivo automaticamente.

Não é preciso (nem é possível, do jeito que o projeto foi feito) editar
nenhum arquivo `.rs` para isso: o motor não tem nenhuma lista de extensões
conhecidas, só a lista de prefixos que foi construída em tempo de execução
a partir do que cada `.lua` registrou.

---

## A consulta ao banco a partir da extensão

Duas funções genéricas são expostas globalmente ao Lua (dentro da tabela
`db`, ao lado de `db.registrar`):

- **`db.buscar(chave) -> valor:string | nil`** — devolve o valor bruto
  gravado sob `chave` agora, ou `nil` se a chave não existe.
- **`db.chaves() -> {chave1, chave2, ...}`** — devolve uma tabela-array com
  todas as chaves gravadas no banco agora.

As duas são implementadas em `extensoes.rs` chamando diretamente
`Armazenamento::buscar`/`Armazenamento::chaves` — nenhuma delas sabe, nem
precisa saber, o que é um prefixo `cpf_`, `data_`, `ref_` ou qualquer outro.
É por isso que a extensão desconhecida da correção, que "usa a consulta ao
banco", funciona sem tocar em uma linha de Rust: ela chama as mesmas duas
funções que o CPF e a Referência (nossa extensão proposta) já chamam.

### O ponto difícil: o `ADD` já está "no meio" de uma operação sobre o mesmo banco

O `Armazenamento` guarda os dados atrás de um `Rc<RefCell<HashMap<..>>>` (não
`Arc<Mutex<..>>`, porque o programa é de uma única thread — não há
concorrência real a proteger, só a necessidade de mutabilidade
compartilhada entre `main`, a ponte e os fechamentos que ela registra no
Lua). Um `RefCell` verifica em tempo de execução que não existe um
empréstimo mutável (`borrow_mut`) e um empréstimo qualquer (`borrow` ou
`borrow_mut`) ativos ao mesmo tempo — se isso acontecer, ele entra em
pânico.

Isso é exatamente o risco descrito no enunciado: no instante em que a
extensão de CPF chama `db.chaves()`/`db.buscar()` para checar unicidade, o
comando `ADD` que disparou essa validação já está, de certa forma, "em
andamento" sobre o mesmo banco. Uma implementação ingênua que fizesse algo
como

```rust
// NÃO é o que este projeto faz — exemplo do que daria errado
let mut dados = self.dados.borrow_mut(); // trava o banco para escrita...
let resultado = chamar_extensao_lua(...); // ...e SÓ DEPOIS chama o Lua,
                                            // que tenta ler o mesmo banco
dados.insert(chave, resultado);
```

entraria em pânico assim que a extensão chamasse `db.buscar` durante a
validação, porque o empréstimo mutável de `dados` ainda estaria ativo
quando o fechamento de `db.buscar` tentasse um empréstimo imutável.

A solução deste projeto é separar completamente as duas fases:

1. **Fase de validação** (`Ponte::validar_add`): chama a função Lua de
   `add`. Durante essa chamada, o Rust **não segura nenhum empréstimo** do
   `RefCell` — cada chamada a `db.buscar`/`db.chaves`, feita de dentro do
   Lua, pega um empréstimo *imutável*, curtíssimo (só a duração daquela
   chamada específica), e o solta antes de devolver o controle ao Lua. Como
   múltiplos empréstimos imutáveis podem coexistir sem problema (é só
   escrita simultânea que é proibida), quantas consultas a extensão fizer
   durante a validação, não há conflito algum.
2. **Fase de gravação** (`Armazenamento::gravar`, chamada só depois que a
   fase 1 termina com sucesso): pega o único empréstimo *mutável* de toda a
   operação, insere o valor, e solta o empréstimo imediatamente.

Como as duas fases nunca se sobrepõem — o empréstimo mutável da fase 2 só é
pedido depois que todo o código Lua da fase 1 já retornou —, nunca existe um
`borrow_mut` ativo no instante em que uma extensão faz uma consulta. Não foi
preciso nenhum mecanismo especial de bloqueio, fila ou cópia: a ordem das
operações, por si só, evita o conflito. Isso também é o motivo de a regra
"regravar o mesmo CPF na mesma chave é permitido" funcionar corretamente: no
momento da consulta, o valor antigo (se houver) ainda está no banco — ele só
é sobrescrito na fase 2 —, e a extensão de CPF exclui explicitamente a
própria chave (`outra_chave ~= chave`) da varredura, então gravar de novo o
mesmo valor na mesma chave nunca colide "consigo mesmo".

---

## Extensões obrigatórias

### CPF (`extensions/cpf.lua`, prefixo `cpf_`)

- **ADD**: exige exatamente 11 dígitos numéricos, sem formatação. Calcula os
  dois dígitos verificadores pelo algoritmo padrão (pesos decrescentes de
  10/11 até 2, resto da divisão por 11) e rejeita à parte qualquer
  sequência de 11 dígitos iguais (que passa no cálculo, mas não é um CPF
  real). Também garante unicidade: se o número já estiver gravado sob outra
  chave, a operação falha dizendo qual é essa chave (usa `db.chaves`/
  `db.buscar`, ver seção anterior).
- **GET**: formata como `000.000.000-00`.

### Data (`extensions/data.lua`, prefixo `data_`)

- **ADD**: exige o formato ISO8601 estrito `aaaa-mm-dd` (via um padrão Lua
  ancorado, `^(%d%d%d%d)%-(%d%d)%-(%d%d)$`, que só casa com exatamente essa
  forma) e que a data exista de verdade — mês entre 01 e 12, dia dentro do
  número de dias daquele mês, com a regra de século dos anos bissextos
  (`ano % 400 == 0` → bissexto; senão, `ano % 100 == 0` → não bissexto;
  senão, `ano % 4 == 0` → bissexto). Não consulta o banco: a validade de uma
  data não depende de nada que já esteja gravado.
- **GET**: formata como `dd/mm/aaaa`.

---

## Extensão proposta pelo grupo: referência entre chaves

**Arquivo:** `extensions/referencia.lua` · **Prefixo:** `ref_` · **Casos de
teste:** `casos_teste_referencia.txt`

**O que faz:** uma chave `ref_*` guarda, como valor, o *nome de outra chave*
já existente no banco — funciona como um ponteiro (ou um link simbólico)
para outra entrada.

- `ADD ref_x outra_chave`: só é aceito se `outra_chave` já existir no banco
  no momento do `ADD` (e se não for a própria chave `ref_x`, o que seria uma
  autorreferência sem utilidade). O que fica gravado é literalmente o nome
  `"outra_chave"`.
- `GET ref_x`: em vez de devolver o nome gravado, a extensão consulta o
  banco **na hora da leitura** (`db.buscar("outra_chave")`) e devolve o
  valor que está *atualmente* sob `outra_chave`.

Por que escolhemos esta extensão e o que ela exercita que CPF e Data não
exercitam:

- **Consulta ao banco durante o `GET`, não só durante o `ADD`.** Das duas
  extensões obrigatórias, só o CPF consulta o banco, e só durante o `ADD`
  (o enunciado é explícito: "o formatador de data valida só o que
  recebe"). A extensão de Referência é a única, das três deste projeto, que
  precisa da consulta também no `GET` — o que prova que `db.buscar`/
  `db.chaves` são funções de consulta de verdade (chamáveis a qualquer
  momento), e não um mecanismo especial só para a fase de `ADD`.
- **Um padrão de consulta diferente.** O CPF pergunta "este *valor* já
  existe, gravado em alguma *outra* chave?" (varre todas as chaves). A
  Referência pergunta "esta *chave*, especificamente, existe?" (uma
  checagem pontual, sem varredura). São os dois padrões de consulta mais
  comuns que se pode fazer a um key-value store, e cada extensão obrigatória
  mais a nossa cobre um deles.
- **Um resultado de `GET` que muda "sozinho".** Como o `GET` de uma
  referência sempre lê o valor atual da chave apontada, é possível fazer
  `ADD` na chave apontada (mudando seu valor) e ver o `GET` da referência
  mudar junto, sem tocar na chave `ref_*` — evidência direta, em tempo de
  execução, de que a consulta ao banco não usa uma cópia (ver
  `casos_teste_referencia.txt`, casos com `origem`/`ref_a`).
- **Não é uma variação de validador de documento.** Ao contrário de CNPJ,
  RG, PIS ou título de eleitor — que seriam "o mesmo exercício do CPF com
  outros pesos" —, uma referência não tem dígito verificador nem checksum:
  a validação inteira é "essa chave existe?", e a transformação do `GET` é
  uma segunda consulta, não uma formatação de string.

Decisão de projeto explícita: a dereferenciação é de **um único nível**. Se
`ref_e` aponta para `ref_a`, e `ref_a` aponta para `origem`, então
`GET ref_e` devolve o valor bruto gravado em `ref_a` — que é o texto
`"origem"` — e não segue a cadeia até chegar ao valor de `origem`. Isso
evita ter que lidar com ciclos (`ref_x` apontando, direta ou indiretamente,
para `ref_x`) e mantém a extensão simples; documentamos essa limitação aqui
e no próprio arquivo `.lua` para deixar claro que é intencional, não um
descuido.

---

## Decisões de projeto

O enunciado deixa várias escolhas em aberto. Estas são as que tomamos, e por quê:

- **`ADD chave` sem nenhum valor depois (nem um espaço) não é erro de
  sintaxe — o valor vira a string vazia.** Só `ADD` sozinho (sem nenhuma
  chave) é "comando incompleto". Isso é o que faz `ADD cpf_k` (sem valor
  nenhum) chegar até a extensão de CPF como um valor vazio, que ela então
  rejeita com seu próprio motivo ("valor vazio") — em vez de um genérico
  "comando incompleto" vindo do parser, que teria que "adivinhar" que toda
  chave sem valor é um erro, mesmo para chaves sem extensão nenhuma (onde
  gravar uma string vazia é uma operação perfeitamente razoável).
- **Chaves sem nenhuma extensão registrada aceitam qualquer valor, sem
  validação nem formatação**, incluindo string vazia. O enunciado só define
  comportamento para `cpf_*`/`data_*`; para tudo o mais, o banco se comporta
  como um key-value store comum.
- **Casamento de prefixo é por `starts_with`, e o mais específico (mais
  longo) vence em caso de ambiguidade.** Uma chave só "pertence" a uma
  extensão se literalmente começar com o prefixo registrado por ela — é
  assim que `cpfx` (que só "lembra" o prefixo `cpf_`, mas não é seguido de
  `_`) passa direto, sem validação, como pede o `casos_teste.txt`.
- **`add`/`get` são independentes na tabela de registro.** Ver a seção do
  protocolo — permite que uma extensão implemente só validação (como
  poderia ser o caso de uma extensão de "campo obrigatório", sem
  formatação nenhuma no `GET`) ou só formatação (um valor livre, mas
  exibido de um jeito específico), sem ser forçada a fornecer as duas.
- **Fim da entrada (EOF, quando a entrada vem de um pipe) tem o mesmo efeito
  de `EXIT`**, conforme pedido explicitamente no enunciado — implementado
  checando se `read_line` devolveu `0` bytes lidos.
- **O prompt (`> `) é sempre impresso**, tanto no modo interativo quanto
  quando a entrada vem de um pipe. O enunciado não pede para suprimi-lo no
  modo pipe, e a transcrição de exemplo no próprio enunciado mostra o
  prompt sendo impresso antes de cada comando.
- **Campos de registro malformados (por exemplo, `add` presente mas não
  sendo uma função) são tratados como ausentes**, em vez de derrubar o
  carregamento de todas as extensões por causa de uma só malformada. Isso
  favorece o banco continuar funcionando (com as outras extensões
  intactas) mesmo que uma extensão de terceiros tenha um bug de registro.
- **Erros de execução do Lua (`error(...)`, indexação de `nil`, retorno em
  formato inesperado) viram `ERRO: erro na extensão: <mensagem do mlua>`**,
  em vez de derrubar o processo. Isso é o que garante a promessa de "nenhuma
  entrada derruba o programa" mesmo quando a "entrada" é, na verdade, um bug
  numa extensão de terceiros.
- **`Rc<RefCell<..>>` em vez de `Arc<Mutex<..>>`** para o armazenamento: o
  programa não usa threads (é um laço único lendo comandos), então a
  sincronização entre threads do `Mutex` seria custo sem benefício — o que
  precisávamos era só de mutabilidade compartilhada entre `main`, a ponte e
  os fechamentos Lua, que é exatamente o que `Rc<RefCell<..>>` fornece.

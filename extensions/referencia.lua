-- Referência entre chaves.
--
-- Trata chaves com o prefixo "ref_". O valor gravado sob uma chave "ref_*"
-- não é um dado "de verdade": é o NOME de uma outra chave já existente no
-- banco. Pense nela como um ponteiro, ou um link simbólico.
--
-- ADD: valida que o valor recebido é o nome de uma chave que já existe no
-- banco (e que não é a própria chave sendo gravada — referenciar a si mesmo
-- não tem utilidade e é rejeitado). O que fica gravado é o nome da chave
-- referenciada, tal como recebido — sem transformação nenhuma.
--
-- GET: em vez de devolver o nome gravado, a extensão consulta o banco NA
-- HORA da leitura (com db.buscar) e devolve o valor bruto que está
-- atualmente sob a chave referenciada. Ou seja, o GET reflete o estado
-- ATUAL do banco, não uma cópia tirada no momento do ADD: se o valor da
-- chave referenciada mudar depois, a referência "acompanha" a mudança.
--
-- A dereferenciação é de um único nível: se a chave referenciada for, ela
-- própria, uma referência, o valor devolvido é o texto bruto gravado nela
-- (isto é, o NOME de uma terceira chave), sem seguir a cadeia adiante. Essa
-- é uma escolha de projeto deliberada — ver o README — que mantém a
-- extensão simples e evita qualquer necessidade de detectar ciclos.
--
-- O que esta extensão exercita que nem o CPF nem a Data exercitam:
-- - Consulta ao banco durante o GET, e não só durante o ADD (o formatador
--   de Data nunca consulta o banco; o CPF só consulta durante o ADD).
-- - Um tipo de consulta diferente: "esta chave existe?" (checagem pontual
--   de uma chave específica), em vez de "este valor já existe em alguma
--   outra chave?" (varredura de todas as chaves, como o CPF faz).
-- - Um GET cujo resultado pode mudar entre duas chamadas sem que a própria
--   chave "ref_*" tenha sido regravada — evidência direta de que
--   db.buscar/db.chaves realmente consultam o banco ao vivo, e não uma
--   cópia tirada em algum momento anterior.

local function ao_adicionar(chave, valor)
    if valor == "" then
        return false, "valor vazio"
    end

    if valor == chave then
        return false, "não é permitido referenciar a própria chave"
    end

    if db.buscar(valor) == nil then
        return false, "chave referenciada não existe: " .. valor
    end

    return true, valor
end

local function ao_buscar(_chave, valor)
    local atual = db.buscar(valor)
    if atual == nil then
        -- Só pode acontecer se, no futuro, este banco ganhar um comando de
        -- remoção: hoje não há como uma chave deixar de existir depois de
        -- criada. A checagem fica aqui mesmo assim, por robustez.
        return false, "chave referenciada não existe mais: " .. valor
    end
    return true, atual
end

db.registrar("ref_", {
    add = ao_adicionar,
    get = ao_buscar,
})

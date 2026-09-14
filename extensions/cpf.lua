-- Validador de CPF.
--
-- Trata chaves com o prefixo "cpf_".
--
-- ADD: a entrada deve conter exatamente 11 dígitos numéricos, sem nenhuma
-- formatação (sem pontos, sem traço). Além do dígito verificador (ver
-- referência no enunciado), sequências de dígito repetido (111.111.111-11,
-- 000.000.000-00 etc.) são rejeitadas à parte: elas passam no cálculo do
-- dígito verificador, mas não são CPFs válidos.
--
-- ADD, unicidade: o mesmo número de CPF não pode ficar gravado sob duas
-- chaves diferentes. Regravar o mesmo número na MESMA chave é permitido.
-- Esta é a única regra, entre as três extensões deste projeto, que precisa
-- perguntar ao banco "esse valor já existe em outro lugar?" — por isso ela
-- usa db.chaves()/db.buscar() para varrer o banco inteiro.
--
-- GET: formata para "000.000.000-00". O valor gravado continua sendo os 11
-- dígitos crus; a formatação acontece só na leitura.

local function somente_digitos(valor)
    return valor:match("^%d+$") ~= nil
end

local function todos_os_digitos_iguais(digitos)
    local primeiro = digitos:sub(1, 1)
    for i = 2, #digitos do
        if digitos:sub(i, i) ~= primeiro then
            return false
        end
    end
    return true
end

-- Calcula um dígito verificador de CPF a partir dos primeiros `quantidade`
-- dígitos (9 para o primeiro dígito verificador, 10 para o segundo,
-- incluindo o primeiro já calculado).
local function calcular_digito_verificador(digitos, quantidade)
    local soma = 0
    local peso = quantidade + 1
    for i = 1, quantidade do
        local digito = tonumber(digitos:sub(i, i))
        soma = soma + digito * peso
        peso = peso - 1
    end
    local resto = soma % 11
    if resto < 2 then
        return 0
    end
    return 11 - resto
end

-- Retorna true/false e, se false, o motivo. Assume que `digitos` já tem
-- exatamente 11 caracteres, todos dígitos.
local function cpf_e_valido(digitos)
    if todos_os_digitos_iguais(digitos) then
        return false, "sequência de dígitos repetidos não é um CPF válido"
    end

    local esperado_10 = calcular_digito_verificador(digitos:sub(1, 9), 9)
    local esperado_11 = calcular_digito_verificador(digitos:sub(1, 10), 10)

    local informado_10 = tonumber(digitos:sub(10, 10))
    local informado_11 = tonumber(digitos:sub(11, 11))

    if informado_10 ~= esperado_10 or informado_11 ~= esperado_11 then
        return false, "dígito verificador inválido"
    end

    return true
end

-- Procura, entre todas as chaves do banco (exceto a própria chave sendo
-- gravada), alguma que já guarde este mesmo CPF. Devolve o nome dessa
-- chave, ou nil se não encontrar nenhuma.
local function chave_com_mesmo_cpf(chave_atual, cpf)
    for _, outra_chave in ipairs(db.chaves()) do
        if outra_chave ~= chave_atual and db.buscar(outra_chave) == cpf then
            return outra_chave
        end
    end
    return nil
end

local function ao_adicionar(chave, valor)
    if valor == "" then
        return false, "valor vazio"
    end

    if not somente_digitos(valor) then
        return false, "a entrada deve conter apenas números, sem formatação"
    end

    if #valor ~= 11 then
        return false, string.format("%d dígitos informados, esperados 11", #valor)
    end

    local ok, motivo = cpf_e_valido(valor)
    if not ok then
        return false, motivo
    end

    local chave_colidindo = chave_com_mesmo_cpf(chave, valor)
    if chave_colidindo ~= nil then
        return false, "CPF já cadastrado na chave " .. chave_colidindo
    end

    return true, valor
end

local function ao_buscar(_chave, valor)
    local formatado = valor:sub(1, 3) .. "."
        .. valor:sub(4, 6) .. "."
        .. valor:sub(7, 9) .. "-"
        .. valor:sub(10, 11)
    return true, formatado
end

db.registrar("cpf_", {
    add = ao_adicionar,
    get = ao_buscar,
})

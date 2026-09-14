-- Formatador de Data.
--
-- Trata chaves com o prefixo "data_".
--
-- ADD, formato: a entrada deve estar em ISO8601 estrito, exatamente
-- "aaaa-mm-dd" (quatro dígitos de ano, dois de mês, dois de dia, com zero à
-- esquerda). Formatos frouxos como "2023-1-5" são inválidos.
--
-- ADD, calendário: além do formato, a data tem que existir de verdade — mês
-- entre 01 e 12, e dia dentro do número de dias daquele mês naquele ano.
--
-- ADD, ano bissexto: um ano é bissexto se for divisível por 4, EXCETO
-- quando for divisível por 100 e não for divisível por 400 — por isso 1900
-- e 2100 não são bissextos, mas 2000 e 2024 são. Testar só "ano % 4 == 0"
-- erra o ano 1900.
--
-- Esta extensão valida só o valor recebido: ao contrário do CPF, ela nunca
-- consulta o banco (a validade de uma data não depende de nada mais que já
-- esteja gravado).
--
-- GET: formata para o padrão brasileiro dd/mm/aaaa.

local DIAS_NO_MES = { 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31 }

local function e_bissexto(ano)
    if ano % 400 == 0 then
        return true
    end
    if ano % 100 == 0 then
        return false
    end
    return ano % 4 == 0
end

local function dias_no_mes(mes, ano)
    if mes == 2 and e_bissexto(ano) then
        return 29
    end
    return DIAS_NO_MES[mes]
end

local function ao_adicionar(_chave, valor)
    if valor == "" then
        return false, "valor vazio"
    end

    -- Formato estrito: 4 dígitos, hífen, 2 dígitos, hífen, 2 dígitos —
    -- nada mais, nada a menos (as âncoras ^ e $ garantem isso).
    local ano_str, mes_str, dia_str = valor:match("^(%d%d%d%d)%-(%d%d)%-(%d%d)$")
    if ano_str == nil then
        return false, "formato inválido, esperado aaaa-mm-dd (ISO8601)"
    end

    local ano = tonumber(ano_str)
    local mes = tonumber(mes_str)
    local dia = tonumber(dia_str)

    if mes < 1 or mes > 12 then
        return false, "mês fora da faixa (deve estar entre 01 e 12)"
    end

    local maximo_de_dias = dias_no_mes(mes, ano)
    if dia < 1 or dia > maximo_de_dias then
        return false, string.format(
            "dia fora da faixa: o mês informado tem %d dias", maximo_de_dias
        )
    end

    return true, valor
end

local function ao_buscar(_chave, valor)
    local ano, mes, dia = valor:match("^(%d%d%d%d)%-(%d%d)%-(%d%d)$")
    return true, dia .. "/" .. mes .. "/" .. ano
end

db.registrar("data_", {
    add = ao_adicionar,
    get = ao_buscar,
})

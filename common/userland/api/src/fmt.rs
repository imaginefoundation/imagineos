//! Formatação numérica sem alocação.
//!
//! O shell e os utilitários não têm alocador nem `core::fmt` de ponto flutuante,
//! então conversões são feitas com divisão inteira sobre buffers na pilha. Estar
//! em um módulo só evita que cada utilitário reimplemente (e erre) os mesmos
//! detalhes — como zeros à esquerda em `0.01`.

/// Escreve `value` em decimal no buffer, devolvendo a fatia usada.
pub fn decimal(output: &mut [u8], value: u64) -> &[u8] {
    let mut buffer = [0u8; 20];
    let mut cursor = buffer.len();
    let mut remaining = value;
    loop {
        cursor -= 1;
        buffer[cursor] = b'0' + (remaining % 10) as u8;
        remaining /= 10;
        if remaining == 0 {
            break;
        }
    }
    let digits = &buffer[cursor..];
    let length = digits.len().min(output.len());
    output[..length].copy_from_slice(&digits[..length]);
    &output[..length]
}

/// Escreve `value` no console.
pub fn write_decimal(value: u64) {
    let mut buffer = [0u8; 20];
    let _ = crate::console::write_all(decimal(&mut buffer, value));
}

/// Escreve `value` em hexadecimal com `min_digits` casas (`0x` não incluso).
pub fn write_hex(value: u64, min_digits: usize) {
    let mut buffer = [0u8; 16];
    let mut cursor = buffer.len();
    let mut remaining = value;
    loop {
        cursor -= 1;
        let digit = (remaining & 0xf) as u8;
        buffer[cursor] = if digit < 10 {
            b'0' + digit
        } else {
            b'a' + digit - 10
        };
        remaining >>= 4;
        if remaining == 0 {
            break;
        }
    }
    while buffer.len() - cursor < min_digits {
        cursor -= 1;
        buffer[cursor] = b'0';
    }
    let _ = crate::console::write_all(&buffer[cursor..]);
}

/// Escreve `value` com `decimals` casas decimais, usando divisão inteira.
///
/// `write_fixed(1234, 2)` escreve `12.34`; `write_fixed(5, 2)` escreve `0.05`
/// (os zeros à esquerda são o detalhe que se erra com facilidade).
pub fn write_fixed(value: u64, decimals: u32) {
    let mut divisor = 1u64;
    for _ in 0..decimals {
        divisor *= 10;
    }
    write_decimal(value / divisor);
    if decimals == 0 {
        return;
    }
    let _ = crate::console::write_all(b".");
    let fraction = value % divisor;
    let mut leading = divisor / 10;
    while leading > 1 {
        if fraction >= leading {
            break;
        }
        let _ = crate::console::write_all(b"0");
        leading /= 10;
    }
    let mut buffer = [0u8; 20];
    let _ = crate::console::write_all(decimal(&mut buffer, fraction));
}

/// Escreve `bytes` com padding à direita até `width` colunas.
pub fn write_padded(bytes: &[u8], width: usize) {
    let _ = crate::console::write_all(bytes);
    for _ in bytes.len()..width {
        let _ = crate::console::write_all(b" ");
    }
}

/// Escreve um número com padding à esquerda até `width` colunas.
pub fn write_decimal_padded(value: u64, width: usize) {
    let mut buffer = [0u8; 20];
    let digits = decimal(&mut buffer, value);
    for _ in digits.len()..width {
        let _ = crate::console::write_all(b" ");
    }
    let _ = crate::console::write_all(digits);
}

/// Converte bytes de um campo de tamanho fixo em `&str`, cortando no NUL.
pub fn field_text(field: &[u8]) -> &str {
    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());
    core::str::from_utf8(&field[..end]).unwrap_or("")
}

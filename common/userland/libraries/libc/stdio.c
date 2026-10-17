#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <fcntl.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define OUTPUT_CAPACITY 512

typedef struct {
    int fd;
    int error;
    int eof;
} dreamcore_file_t;

FILE *stdin = (FILE *)0;
FILE *stdout = (FILE *)1;
FILE *stderr = (FILE *)2;

static int stream_write(FILE *stream, const void *buffer, size_t length)
{
    if (!stream) stream = stdout;
    int descriptor = (unsigned long)stream <= 2UL
        ? (int)(unsigned long)stream
        : ((dreamcore_file_t *)stream)->fd;
    const char *cursor = buffer;
    size_t written = 0;
    while (written < length) {
        size_t chunk = length - written;
        if (chunk > 4096) chunk = 4096;
        long result = write(descriptor, cursor + written, chunk);
        if (result < 0) {
            if ((unsigned long)stream > 2UL) ((dreamcore_file_t *)stream)->error = 1;
            return written ? (int)written : -1;
        }
        if (result == 0) break;
        written += (size_t)result;
    }
    return (int)written;
}

static int stream_read(FILE *stream, void *buffer, size_t length)
{
    if (!stream) stream = stdin;
    int descriptor = (unsigned long)stream <= 2UL
        ? (int)(unsigned long)stream
        : ((dreamcore_file_t *)stream)->fd;
    char *cursor = buffer;
    size_t received = 0;
    while (received < length) {
        size_t chunk = length - received;
        if (chunk > 4096) chunk = 4096;
        long result = read(descriptor, cursor + received, chunk);
        if (result < 0) {
            if ((unsigned long)stream > 2UL) ((dreamcore_file_t *)stream)->error = 1;
            return received ? (int)received : (int)result;
        }
        if (result == 0) {
            if ((unsigned long)stream > 2UL) ((dreamcore_file_t *)stream)->eof = 1;
            return (int)received;
        }
        received += (size_t)result;
    }
    return (int)received;
}

static void emit_char(char *buffer, size_t *used, size_t capacity, char value)
{
    if (capacity && *used < capacity - 1) buffer[*used] = value;
    if (*used < (size_t)-1) (*used)++;
}

static void emit_repeat(char *buffer, size_t *used, size_t capacity, char value, size_t count)
{
    while (count--) emit_char(buffer, used, capacity, value);
}

static void emit_bytes(char *buffer, size_t *used, size_t capacity, const char *text, size_t length)
{
    while (length--) emit_char(buffer, used, capacity, *text++);
}

enum FormatLength {
    LENGTH_DEFAULT,
    LENGTH_CHAR,
    LENGTH_SHORT,
    LENGTH_LONG,
    LENGTH_LONG_LONG,
    LENGTH_SIZE,
    LENGTH_PTRDIFF,
    LENGTH_INTMAX,
};

static int format_write(char *buffer, size_t capacity, const char *format, va_list arguments)
{
    size_t used = 0;
    while (*format) {
        if (*format != '%') {
            emit_char(buffer, &used, capacity, *format++);
            continue;
        }
        format++;
        if (*format == '%') {
            emit_char(buffer, &used, capacity, '%');
            format++;
            continue;
        }
        int left = 0, plus = 0, space = 0, alternate = 0, zero = 0;
        int reading_flags = 1;
        while (reading_flags) {
            switch (*format) {
            case '-': left = 1; format++; break;
            case '+': plus = 1; format++; break;
            case ' ': space = 1; format++; break;
            case '#': alternate = 1; format++; break;
            case '0': zero = 1; format++; break;
            default: reading_flags = 0; break;
            }
        }
        int width = 0;
        if (*format == '*') {
            width = va_arg(arguments, int);
            format++;
            if (width < 0) {
                left = 1;
                width = -width;
            }
        } else {
            while (*format >= '0' && *format <= '9') {
                if (width < INT_MAX / 10) width = width * 10 + (*format - '0');
                format++;
            }
        }
        int precision = -1;
        if (*format == '.') {
            format++;
            precision = 0;
            if (*format == '*') {
                precision = va_arg(arguments, int);
                format++;
                if (precision < 0) precision = -1;
            } else {
                while (*format >= '0' && *format <= '9') {
                    if (precision < INT_MAX / 10) precision = precision * 10 + (*format - '0');
                    format++;
                }
            }
        }
        enum FormatLength length = LENGTH_DEFAULT;
        if (*format == 'h') {
            format++;
            if (*format == 'h') {
                format++;
                length = LENGTH_CHAR;
            } else {
                length = LENGTH_SHORT;
            }
        } else if (*format == 'l') {
            format++;
            if (*format == 'l') {
                format++;
                length = LENGTH_LONG_LONG;
            } else {
                length = LENGTH_LONG;
            }
        } else if (*format == 'z') {
            format++;
            length = LENGTH_SIZE;
        } else if (*format == 't') {
            format++;
            length = LENGTH_PTRDIFF;
        } else if (*format == 'j') {
            format++;
            length = LENGTH_INTMAX;
        } else if (*format == 'L') {
            format++;
            length = LENGTH_LONG_LONG;
        }
        char conversion = *format ? *format++ : 0;
        if (conversion == 's') {
            const char *text = va_arg(arguments, const char *);
            if (!text) text = "(null)";
            size_t text_length = strlen(text);
            if (precision >= 0 && text_length > (size_t)precision) text_length = (size_t)precision;
            if (!left && width > (int)text_length)
                emit_repeat(buffer, &used, capacity, ' ', (size_t)width - text_length);
            emit_bytes(buffer, &used, capacity, text, text_length);
            if (left && width > (int)text_length)
                emit_repeat(buffer, &used, capacity, ' ', (size_t)width - text_length);
            continue;
        }
        if (conversion == 'c') {
            char value = (char)va_arg(arguments, int);
            if (!left && width > 1) emit_repeat(buffer, &used, capacity, ' ', (size_t)width - 1);
            emit_char(buffer, &used, capacity, value);
            if (left && width > 1) emit_repeat(buffer, &used, capacity, ' ', (size_t)width - 1);
            continue;
        }
        if (conversion == 'm') {
            const char *message = strerror(errno);
            size_t text_length = strlen(message);
            if (!left && width > (int)text_length)
                emit_repeat(buffer, &used, capacity, ' ', (size_t)width - text_length);
            emit_bytes(buffer, &used, capacity, message, text_length);
            if (left && width > (int)text_length)
                emit_repeat(buffer, &used, capacity, ' ', (size_t)width - text_length);
            continue;
        }
        if (conversion == 'n') {
            if (length == LENGTH_LONG) *va_arg(arguments, long *) = (long)used;
            else if (length == LENGTH_LONG_LONG) *va_arg(arguments, long long *) = (long long)used;
            else if (length == LENGTH_SIZE) *va_arg(arguments, size_t *) = used;
            else *va_arg(arguments, int *) = (int)used;
            continue;
        }
        unsigned base = conversion == 'o' ? 8
            : (conversion == 'x' || conversion == 'X' || conversion == 'p') ? 16 : 10;
        int signed_conversion = conversion == 'd' || conversion == 'i';
        int pointer_conversion = conversion == 'p';
        if (!signed_conversion && conversion != 'u' && conversion != 'o'
            && conversion != 'x' && conversion != 'X' && !pointer_conversion) {
            emit_char(buffer, &used, capacity, '%');
            if (conversion) emit_char(buffer, &used, capacity, conversion);
            else break;
            continue;
        }
        int negative = 0;
        uintmax_t value;
        if (pointer_conversion) {
            value = (uintptr_t)va_arg(arguments, void *);
        } else if (signed_conversion) {
            intmax_t signed_value;
            switch (length) {
            case LENGTH_LONG: signed_value = va_arg(arguments, long); break;
            case LENGTH_LONG_LONG: signed_value = va_arg(arguments, long long); break;
            case LENGTH_SIZE:
            case LENGTH_PTRDIFF: signed_value = va_arg(arguments, ptrdiff_t); break;
            case LENGTH_INTMAX: signed_value = va_arg(arguments, intmax_t); break;
            default: signed_value = va_arg(arguments, int); break;
            }
            negative = signed_value < 0;
            value = negative ? (uintmax_t)(-(signed_value + 1)) + 1 : (uintmax_t)signed_value;
        } else {
            switch (length) {
            case LENGTH_LONG: value = va_arg(arguments, unsigned long); break;
            case LENGTH_LONG_LONG: value = va_arg(arguments, unsigned long long); break;
            case LENGTH_SIZE: value = va_arg(arguments, size_t); break;
            case LENGTH_PTRDIFF: value = (uintmax_t)va_arg(arguments, ptrdiff_t); break;
            case LENGTH_INTMAX: value = va_arg(arguments, uintmax_t); break;
            default: value = va_arg(arguments, unsigned int); break;
            }
        }
        char digits[sizeof(uintmax_t) * 8];
        size_t digit_count = 0;
        const char *alphabet = conversion == 'X' ? "0123456789ABCDEF" : "0123456789abcdef";
        if (value || precision != 0) {
            do {
                digits[digit_count++] = alphabet[value % base];
                value /= base;
            } while (value);
        }
        size_t zero_count = precision > (int)digit_count
            ? (size_t)precision - digit_count : 0;
        char sign = negative ? '-' : plus ? '+' : space ? ' ' : 0;
        const char *prefix = "";
        size_t prefix_length = 0;
        if (pointer_conversion || (alternate && digit_count && (base == 16 || base == 8))) {
            prefix = base == 16 ? (conversion == 'X' ? "0X" : "0x") : "0";
            prefix_length = base == 16 ? 2 : 1;
        }
        size_t content_length = digit_count + zero_count + prefix_length + (sign != 0);
        size_t padding = width > (int)content_length ? (size_t)width - content_length : 0;
        if (!left && !(zero && precision < 0)) emit_repeat(buffer, &used, capacity, ' ', padding);
        if (sign) emit_char(buffer, &used, capacity, sign);
        emit_bytes(buffer, &used, capacity, prefix, prefix_length);
        if (!left && zero && precision < 0) emit_repeat(buffer, &used, capacity, '0', padding);
        emit_repeat(buffer, &used, capacity, '0', zero_count);
        while (digit_count) emit_char(buffer, &used, capacity, digits[--digit_count]);
        if (left) emit_repeat(buffer, &used, capacity, ' ', padding);
    }
    if (capacity > 0) buffer[used < capacity ? used : capacity - 1] = 0;
    return used > INT_MAX ? INT_MAX : (int)used;
}

static int flush_output(FILE *stream, const char *buffer, size_t length)
{
    return stream_write(stream, buffer, length);
}

int vprintf(const char *format, va_list arguments)
{
    return vfprintf(stdout, format, arguments);
}

int printf(const char *format, ...)
{
    va_list arguments;
    va_start(arguments, format);
    int result = vprintf(format, arguments);
    va_end(arguments);
    return result;
}

int puts(const char *text)
{
    size_t length = strlen(text);
    int wrote = flush_output(stdout, text, length);
    if (wrote < 0 || (size_t)wrote != length) return EOF;
    return flush_output(stdout, "\n", 1) < 0 ? EOF : 0;
}

int putchar(int character)
{
    char byte = (unsigned char)character;
    return flush_output(stdout, &byte, 1) == 1 ? character : EOF;
}

int getchar(void)
{
    unsigned char byte;
    long result = read(STDIN_FILENO, &byte, 1);
    return result == 1 ? (int)byte : EOF;
}

size_t fwrite(const void *buffer, size_t size, size_t count, FILE *stream)
{
    if (size && count > (size_t)-1 / size) return 0;
    size_t total = size * count;
    if (!buffer || !total) return 0;
    int result = stream_write(stream, buffer, total);
    return result < 0 ? 0 : (size_t)result / size;
}

size_t fread(void *buffer, size_t size, size_t count, FILE *stream)
{
    if (size && count > (size_t)-1 / size) return 0;
    size_t total = size * count;
    if (!buffer || !total) return 0;
    long result = stream_read(stream, buffer, total);
    return result < 0 ? 0 : (size_t)result / size;
}

int fputc(int character, FILE *stream)
{
    char value = (char)character;
    return stream_write(stream, &value, 1) == 1 ? character : EOF;
}

int fputs(const char *text, FILE *stream)
{
    if (!text) return EOF;
    return stream_write(stream, text, strlen(text)) >= 0 ? 0 : EOF;
}

FILE *fopen(const char *path, const char *mode)
{
    if (!path || !mode || !mode[0]) return 0;
    int flags;
    if (mode[0] == 'r') flags = O_RDONLY;
    else if (mode[0] == 'w') flags = O_WRONLY | O_CREAT | O_TRUNC;
    else if (mode[0] == 'a') flags = O_WRONLY | O_CREAT | O_APPEND;
    else {
        errno = 22;
        return 0;
    }
    for (const char *cursor = mode + 1; *cursor; cursor++) {
        if (*cursor == '+') flags = (flags & ~O_ACCMODE) | O_RDWR;
        else if (*cursor != 'b') {
            errno = 22;
            return 0;
        }
    }
    int fd = open(path, flags, 0644);
    if (fd < 0) return 0;
    dreamcore_file_t *file = malloc(sizeof(*file));
    if (!file) {
        close(fd);
        return 0;
    }
    file->fd = fd;
    file->error = 0;
    file->eof = 0;
    return (FILE *)file;
}

FILE *fdopen(int descriptor, const char *mode)
{
    if (descriptor < 0 || !mode || !mode[0]) {
        errno = 22;
        return 0;
    }

    dreamcore_file_t *file = malloc(sizeof(*file));
    if (!file) return 0;
    file->fd = descriptor;
    file->error = 0;
    file->eof = 0;
    return (FILE *)file;
}

FILE *freopen(const char *path, const char *mode, FILE *stream)
{
    FILE *replacement = fopen(path, mode);
    if (!replacement) return 0;
    if (stream == stdin) stdin = replacement;
    else if (stream == stdout) stdout = replacement;
    else if (stream == stderr) stderr = replacement;
    else if (stream) fclose(stream);
    return replacement;
}

int fileno(FILE *stream)
{
    if (!stream) {
        errno = 22;
        return -1;
    }
    return (unsigned long)stream <= 2UL
        ? (int)(unsigned long)stream
        : ((dreamcore_file_t *)stream)->fd;
}

int fclose(FILE *stream)
{
    if (!stream) return EOF;
    dreamcore_file_t *file = (dreamcore_file_t *)stream;
    int result = close(file->fd);
    free(file);
    return result == 0 ? 0 : EOF;
}

int fseek(FILE *stream, long offset, int whence)
{
    int descriptor = fileno(stream);
    if (descriptor < 0) return -1;
    if ((unsigned long)stream <= 2UL) {
        errno = 29;
        return -1;
    }
    return lseek(descriptor, offset, whence) < 0 ? -1 : 0;
}

long ftell(FILE *stream)
{
    int descriptor = fileno(stream);
    if (descriptor < 0 || (unsigned long)stream <= 2UL) return -1;
    return lseek(descriptor, 0, SEEK_CUR);
}

void rewind(FILE *stream)
{
    (void)fseek(stream, 0, SEEK_SET);
    clearerr(stream);
}

int ferror(FILE *stream)
{
    return stream && (unsigned long)stream > 2UL
        ? ((dreamcore_file_t *)stream)->error
        : 0;
}

int feof(FILE *stream)
{
    return stream && (unsigned long)stream > 2UL
        ? ((dreamcore_file_t *)stream)->eof
        : 0;
}

void clearerr(FILE *stream)
{
    if (stream && (unsigned long)stream > 2UL) {
        ((dreamcore_file_t *)stream)->error = 0;
        ((dreamcore_file_t *)stream)->eof = 0;
    }
}

int fflush(FILE *stream)
{
    (void)stream;
    return 0;
}

int fgetc(FILE *stream)
{
    unsigned char byte;
    int result = stream_read(stream, &byte, 1);
    return result == 1 ? (int)byte : EOF;
}

char *fgets(char *buffer, int length, FILE *stream)
{
    if (!buffer || length <= 0) return 0;
    int used = 0;
    while (used + 1 < length) {
        int value = fgetc(stream);
        if (value == EOF) break;
        buffer[used++] = (char)value;
        if (value == '\n') break;
    }
    if (used == 0) return 0;
    buffer[used] = 0;
    return buffer;
}

int vfprintf(FILE *stream, const char *format, va_list arguments)
{
    char local[OUTPUT_CAPACITY];
    va_list copy;
    va_copy(copy, arguments);
    int required = format_write(local, sizeof(local), format, copy);
    va_end(copy);
    if (required < 0) return -1;
    if ((size_t)required < sizeof(local)) {
        int result = flush_output(stream, local, (size_t)required);
        return result == required ? result : -1;
    }
    char *buffer = malloc((size_t)required + 1);
    if (!buffer) return -1;
    va_copy(copy, arguments);
    int written = format_write(buffer, (size_t)required + 1, format, copy);
    va_end(copy);
    int result = written == required
        ? flush_output(stream, buffer, (size_t)written)
        : -1;
    free(buffer);
    return result == required ? result : -1;
}

int fprintf(FILE *stream, const char *format, ...)
{
    va_list arguments;
    va_start(arguments, format);
    int result = vfprintf(stream, format, arguments);
    va_end(arguments);
    return result;
}

int snprintf(char *buffer, size_t capacity, const char *format, ...)
{
    va_list arguments;
    va_start(arguments, format);
    int written = format_write(buffer, capacity, format, arguments);
    va_end(arguments);
    return written;
}

int vsnprintf(char *buffer, size_t capacity, const char *format, va_list arguments)
{
    return format_write(buffer, capacity, format, arguments);
}

int sprintf(char *buffer, const char *format, ...)
{
    va_list arguments;
    va_start(arguments, format);
    int written = vsnprintf(buffer, 4096, format, arguments);
    va_end(arguments);
    return written;
}

int vsprintf(char *buffer, const char *format, va_list arguments)
{
    return vsnprintf(buffer, 4096, format, arguments);
}

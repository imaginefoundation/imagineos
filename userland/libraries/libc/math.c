#include <math.h>
#include <limits.h>

double fabs(double value)
{
    return value < 0.0 ? -value : value;
}

double trunc(double value)
{
    if (value > (double)LLONG_MAX || value < (double)LLONG_MIN) return value;
    return (double)(long long)value;
}

double floor(double value)
{
    double integral = trunc(value);
    return integral > value ? integral - 1.0 : integral;
}

double ceil(double value)
{
    double integral = trunc(value);
    return integral < value ? integral + 1.0 : integral;
}

double sqrt(double value)
{
    if (value < 0.0) return NAN;
    if (value == 0.0) return 0.0;
    double estimate = value > 1.0 ? value : 1.0;
    for (int iteration = 0; iteration < 64; iteration++) {
        double next = (estimate + value / estimate) * 0.5;
        if (fabs(next - estimate) < 1e-15) return next;
        estimate = next;
    }
    return estimate;
}

double pow(double base, double exponent)
{
    if (exponent == 0.0) return 1.0;
    long long integer = (long long)exponent;
    if ((double)integer != exponent || integer > 1024 || integer < -1024) return NAN;
    int negative = integer < 0;
    unsigned long long count = negative ? (unsigned long long)(-integer) : (unsigned long long)integer;
    double result = 1.0;
    while (count) {
        if (count & 1) result *= base;
        base *= base;
        count >>= 1;
    }
    return negative ? 1.0 / result : result;
}

double ldexp(double value, int exponent)
{
    if (exponent > 1024 || exponent < -1024) return exponent > 0 ? HUGE_VAL : 0.0;
    while (exponent > 0) {
        value *= 2.0;
        exponent--;
    }
    while (exponent < 0) {
        value *= 0.5;
        exponent++;
    }
    return value;
}

long double ldexpl(long double value, int exponent)
{
    if (exponent > 16384 || exponent < -16384) {
        return exponent > 0 ? (long double)HUGE_VAL : 0.0L;
    }
    while (exponent > 0) {
        value *= 2.0L;
        exponent--;
    }
    while (exponent < 0) {
        value *= 0.5L;
        exponent++;
    }
    return value;
}

double frexp(double value, int *exponent)
{
    if (value == 0.0 || !isfinite(value)) {
        *exponent = 0;
        return value;
    }
    int shift = 0;
    double magnitude = fabs(value);
    while (magnitude >= 1.0) {
        magnitude *= 0.5;
        shift++;
    }
    while (magnitude < 0.5) {
        magnitude *= 2.0;
        shift--;
    }
    *exponent = shift;
    return value < 0.0 ? -magnitude : magnitude;
}

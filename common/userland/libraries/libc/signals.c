#include <errno.h>
#include <signal.h>
#include "dreamcore.h"

extern void dc_signal_restorer(void);

static sighandler_t handlers[32];

int sigaction(int signal_number, const struct sigaction *action, struct sigaction *old_action)
{
    if (signal_number < 1 || signal_number >= (int)(sizeof(handlers) / sizeof(handlers[0]))
        || !action || old_action) {
        errno = old_action ? 38 : 22;
        return -1;
    }
    if (action->sa_flags & SA_SIGINFO) {
        errno = 38;
        return -1;
    }
    long result = dc_syscall3(
        30,
        signal_number,
        (long)action->sa_handler,
        (long)dc_signal_restorer);
    if (result < 0) {
        errno = (int)-result;
        return -1;
    }
    handlers[signal_number] = action->sa_handler;
    return 0;
}

sighandler_t signal(int signal_number, sighandler_t handler)
{
    if (signal_number < 1 || signal_number >= (int)(sizeof(handlers) / sizeof(handlers[0]))) {
        errno = 22;
        return SIG_ERR;
    }
    struct sigaction action = {
        .sa_handler = handler,
        .sa_flags = 0,
        .sa_restorer = dc_signal_restorer,
        .sa_mask = 0,
    };
    sighandler_t previous = handlers[signal_number];
    if (sigaction(signal_number, &action, 0) != 0) return SIG_ERR;
    return previous;
}

int sigemptyset(sigset_t *set)
{
    if (!set) {
        errno = 22;
        return -1;
    }
    *set = 0;
    return 0;
}

int sigaddset(sigset_t *set, int signal_number)
{
    if (!set || signal_number < 1 || signal_number >= 64) {
        errno = 22;
        return -1;
    }
    *set |= 1UL << (signal_number - 1);
    return 0;
}

int sigprocmask(int how, const sigset_t *set, sigset_t *old_set)
{
    (void)how;
    (void)set;
    (void)old_set;
    errno = 38;
    return -1;
}

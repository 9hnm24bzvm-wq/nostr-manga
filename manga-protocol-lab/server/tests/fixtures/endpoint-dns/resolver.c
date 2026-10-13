/* External Linux fault fixture. Loaded only into endpoint_dns's child process.
 * It changes libc resolution for these reserved .invalid names, never production
 * interfaces. The second lookup of pinned.fixture.invalid rebounds to an address
 * where no fixture peer listens. All port-443 connections are refused locally so
 * literal-address policy tests cannot send traffic to the listed destinations. */
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <dlfcn.h>
#include <errno.h>
#include <netdb.h>
#include <stdatomic.h>
#include <string.h>
#include <sys/socket.h>

typedef int (*lookup_fn)(const char *, const char *, const struct addrinfo *, struct addrinfo **);
typedef int (*connect_fn)(int, const struct sockaddr *, socklen_t);
static _Atomic unsigned pinned_lookups;

static int numeric(lookup_fn real, const char *host, const char *service,
                   const struct addrinfo *hints, struct addrinfo **result) {
    struct addrinfo copy = {0};
    if (hints) copy = *hints;
    copy.ai_flags |= AI_NUMERICHOST;
    return real(host, service, &copy, result);
}

int getaddrinfo(const char *host, const char *service, const struct addrinfo *hints,
                struct addrinfo **result) {
    lookup_fn real = (lookup_fn)dlsym(RTLD_NEXT, "getaddrinfo");
    if (!real) return EAI_SYSTEM;
    if (host && strcmp(host, "empty.fixture.invalid") == 0) {
        *result = NULL;
        return 0;
    }
    if (host && strcmp(host, "pinned.fixture.invalid") == 0) {
        const char *address = atomic_fetch_add(&pinned_lookups, 1) == 0
                                  ? "127.0.0.1" : "127.0.0.2";
        return numeric(real, address, service, hints, result);
    }
    if (host && (strcmp(host, "mixed.fixture.invalid") == 0 ||
                 strcmp(host, "many.fixture.invalid") == 0)) {
        int count = strcmp(host, "mixed.fixture.invalid") == 0 ? 2 : 17;
        struct addrinfo **next = result;
        *result = NULL;
        for (int i = 0; i < count; i++) {
            const char *address = count == 2 && i == 1 ? "127.0.0.1" : "1.1.1.1";
            int error = numeric(real, address, service, hints, next);
            if (error) {
                if (*result) freeaddrinfo(*result);
                *result = NULL;
                return error;
            }
            while (*next) next = &(*next)->ai_next;
        }
        return 0;
    }
    return real(host, service, hints, result);
}

int connect(int fd, const struct sockaddr *address, socklen_t length) {
    if (address && ((address->sa_family == AF_INET &&
                    ((const struct sockaddr_in *)address)->sin_port == htons(443)) ||
                   (address->sa_family == AF_INET6 &&
                    ((const struct sockaddr_in6 *)address)->sin6_port == htons(443)))) {
        errno = ECONNREFUSED;
        return -1;
    }
    connect_fn real = (connect_fn)dlsym(RTLD_NEXT, "connect");
    if (!real) { errno = ENOSYS; return -1; }
    return real(fd, address, length);
}

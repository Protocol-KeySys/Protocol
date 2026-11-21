// app.c — the "protected" application. This is the file you will reverse later.
// Build it, throw away the source, and try to write a working keygen
// using only Ghidra/x64dbg.
#include <stdio.h>
#include <string.h>
#include "algo.h"

static int check_key(const char *name, const char *key) {
    unsigned short want[3], got[3];
    char group[5];
    int i;

    // Expect format XXXX-XXXX-XXXX
    if (strlen(key) != 14 || key[4] != '-' || key[9] != '-')
        return 0;

    derive_groups(mix_name(name), want);

    for (i = 0; i < 3; i++) {
        memcpy(group, key + i * 5, 4);
        group[4] = '\0';
        if (!decode_group(group, &got[i])) return 0;
        if (got[i] != want[i]) return 0;
    }
    return 1;
}

static void strip_newline(char *s) {
    s[strcspn(s, "\r\n")] = '\0';
}

int main(void) {
    char name[64], key[32];

    printf("Name: ");
    if (!fgets(name, sizeof name, stdin)) return 1;
    printf("Key:  ");
    if (!fgets(key, sizeof key, stdin)) return 1;
    strip_newline(name);
    strip_newline(key);

    if (check_key(name, key)) {
        puts("License valid. Welcome!");
        return 0;
    }
    puts("Invalid license key.");
    return 1;
}

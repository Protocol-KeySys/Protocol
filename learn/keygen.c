// keygen.c — the vendor side. Generates valid keys for a given name.
// Your job later: recreate this tool WITHOUT this source, by reversing app.exe.
#include <stdio.h>
#include "algo.h"

int main(int argc, char **argv) {
    unsigned short g[3];
    char g0[5], g1[5], g2[5];

    if (argc < 2) {
        fprintf(stderr, "usage: keygen <name>\n");
        return 1;
    }

    derive_groups(mix_name(argv[1]), g);
    encode_group(g[0], g0);
    encode_group(g[1], g1);
    encode_group(g[2], g2);

    printf("%s-%s-%s\n", g0, g1, g2);
    return 0;
}

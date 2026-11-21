// algo.h — the license algorithm. Shared by the vendor keygen and the app.
// When you reverse app.exe later, THIS is what you're trying to rediscover.
#ifndef ALGO_H
#define ALGO_H

// 16-symbol alphabet, deliberately shuffled (not plain hex).
// In disassembly this appears as a lookup table — learn to spot it.
static const char ALPHABET[] = "K7QWZX9MNP3RCTVH";

// FNV-style mixer: the "secret" transform from name -> number.
static unsigned int mix_name(const char *s) {
    unsigned int h = 0xC0FFEEu;
    while (*s)
        h = (h * 33u) ^ (unsigned char)*s++;
    return h;
}

// Split the hash into three 16-bit groups.
// Group 3 is a checksum of the first two — extra work for a reverser.
static void derive_groups(unsigned int h, unsigned short g[3]) {
    g[0] = h & 0xFFFFu;
    g[1] = ((h >> 16) ^ 0x5A5Au) & 0xFFFFu;
    g[2] = (unsigned short)(g[0] + g[1]);
}

// Encode one 16-bit value as 4 alphabet chars (nibble-wise).
static void encode_group(unsigned short v, char out[5]) {
    int i;
    for (i = 0; i < 4; i++)
        out[i] = ALPHABET[(v >> (12 - i * 4)) & 0xF];
    out[4] = '\0';
}

// Decode one 4-char group back to a 16-bit value. Returns 0 on bad char.
static int decode_group(const char in[4], unsigned short *v) {
    int i, j;
    *v = 0;
    for (i = 0; i < 4; i++) {
        int digit = -1;
        for (j = 0; j < 16; j++)
            if (in[i] == ALPHABET[j]) digit = j;
        if (digit < 0) return 0;
        *v = (unsigned short)((*v << 4) | digit);
    }
    return 1;
}

#endif

/*
 * Wii ASND oneshots from the WPACK001 audio TOC (PCM16 mono/stereo).
 *
 * TOC sits after textures + meshes. Pre-audio packs omit the trailing u32
 * audio_n — treat as zero clips. Missing / empty clips never crash.
 */

#include <asndlib.h>
#include <gccore.h>
#include <malloc.h>
#include <stdlib.h>
#include <string.h>

#include "wiimaker_abi.h"

#define WIIMAKER_MAX_CLIPS 32
#define WIIMAKER_AUDIO_Q 16
#define WIIMAKER_CLIP_NAME 48

typedef struct {
    char name[WIIMAKER_CLIP_NAME];
    void *pcm; /* 32-byte aligned, BE PCM16 */
    uint32_t nbytes;
    uint32_t rate;
    uint16_t channels;
} WiimakerClip;

typedef struct {
    uint32_t clip;
    float volume;
} AudioQ;

static WiimakerClip g_clips[WIIMAKER_MAX_CLIPS];
static uint32_t g_clip_n;
static AudioQ g_q[WIIMAKER_AUDIO_Q];
static int g_qn;

static uint16_t rd_u16(const uint8_t **p, const uint8_t *end) {
    if (*p + 2 > end)
        return 0;
    uint16_t v = (uint16_t)(*p)[0] | ((uint16_t)(*p)[1] << 8);
    *p += 2;
    return v;
}

static uint32_t rd_u32(const uint8_t **p, const uint8_t *end) {
    if (*p + 4 > end)
        return 0;
    uint32_t v = (uint32_t)(*p)[0] | ((uint32_t)(*p)[1] << 8) | ((uint32_t)(*p)[2] << 16) |
                 ((uint32_t)(*p)[3] << 24);
    *p += 4;
    return v;
}

static int skip_str(const uint8_t **p, const uint8_t *end) {
    uint16_t n = rd_u16(p, end);
    if (*p + n > end)
        return -1;
    *p += n;
    return 0;
}

static int read_str(const uint8_t **p, const uint8_t *end, char *out, uint32_t cap) {
    uint16_t n = rd_u16(p, end);
    if (*p + n > end)
        return -1;
    if (cap == 0)
        return -1;
    uint16_t copy = n < (uint16_t)(cap - 1) ? n : (uint16_t)(cap - 1);
    memcpy(out, *p, copy);
    out[copy] = '\0';
    *p += n;
    return 0;
}

static int skip_textures(const uint8_t **p, const uint8_t *end, uint32_t n) {
    uint32_t i;
    for (i = 0; i < n; i++) {
        if (skip_str(p, end) != 0)
            return -1;
        (void)rd_u16(p, end); /* w */
        (void)rd_u16(p, end); /* h */
        uint32_t len = rd_u32(p, end);
        if (*p + len > end)
            return -1;
        *p += len;
    }
    return 0;
}

static int skip_meshes(const uint8_t **p, const uint8_t *end, uint32_t n) {
    uint32_t i;
    for (i = 0; i < n; i++) {
        if (skip_str(p, end) != 0)
            return -1;
        uint32_t ilen = rd_u32(p, end);
        if (*p + ilen > end)
            return -1;
        *p += ilen;
        uint32_t icount = rd_u32(p, end);
        uint32_t ibytes = icount * 2u;
        if (*p + ibytes > end)
            return -1;
        *p += ibytes;
    }
    return 0;
}

static void byteswap16(uint16_t *p, uint32_t nbytes) {
    uint32_t n = nbytes / 2u;
    uint32_t i;
    for (i = 0; i < n; i++) {
        uint16_t v = p[i];
        p[i] = (uint16_t)((v >> 8) | (v << 8));
    }
}

static int stem_eq(const char *a, const char *b) {
    /* Compare file stems: beep == beep.wav == path/beep.wav */
    const char *sa = a;
    const char *sb = b;
    const char *p;
    for (p = a; *p; p++) {
        if (*p == '/' || *p == '\\')
            sa = p + 1;
    }
    for (p = b; *p; p++) {
        if (*p == '/' || *p == '\\')
            sb = p + 1;
    }
    while (*sa && *sb && *sa != '.' && *sb != '.' && *sa == *sb) {
        sa++;
        sb++;
    }
    int enda = !*sa || *sa == '.';
    int endb = !*sb || *sb == '.';
    return enda && endb;
}

void wiimaker_audio_shutdown(void) {
    uint32_t i;
    for (i = 0; i < g_clip_n; i++) {
        if (g_clips[i].pcm) {
            free(g_clips[i].pcm);
            g_clips[i].pcm = NULL;
        }
        g_clips[i].nbytes = 0;
        g_clips[i].name[0] = '\0';
    }
    g_clip_n = 0;
    g_qn = 0;
}

uint32_t wiimaker_audio_clip_count(void) { return g_clip_n; }

int wiimaker_audio_find(const char *name) {
    uint32_t i;
    if (!name || !name[0])
        return -1;
    for (i = 0; i < g_clip_n; i++) {
        if (stem_eq(g_clips[i].name, name))
            return (int)i;
    }
    return -1;
}

int wiimaker_audio_play(uint32_t clip_id, float volume) {
    s32 voice;
    s32 fmt;
    s32 vol;
    WiimakerClip *c;

    if (clip_id == WIIMAKER_AUDIO_NO_CLIP || clip_id >= g_clip_n)
        return -1;
    c = &g_clips[clip_id];
    if (!c->pcm || c->nbytes < 2 || c->rate == 0)
        return -1;
    if (c->channels != 1 && c->channels != 2)
        return -1;

    voice = ASND_GetFirstUnusedVoice();
    if (voice < 0)
        return -1;

    if (volume < 0.0f)
        volume = 0.0f;
    if (volume > 1.0f)
        volume = 1.0f;
    vol = (s32)(volume * 255.0f + 0.5f);
    if (vol > 255)
        vol = 255;

    fmt = (c->channels == 2) ? VOICE_STEREO_16BIT : VOICE_MONO_16BIT;
    if (ASND_SetVoice(voice, fmt, (s32)c->rate, 0, c->pcm, (s32)c->nbytes, vol, vol, NULL) !=
        SND_OK)
        return -1;
    return 0;
}

int wiimaker_audio_queue(uint32_t clip_id, float volume) {
    if (g_qn >= WIIMAKER_AUDIO_Q)
        return -1;
    if (clip_id == WIIMAKER_AUDIO_NO_CLIP || clip_id >= g_clip_n)
        return -1;
    g_q[g_qn].clip = clip_id;
    g_q[g_qn].volume = volume;
    g_qn++;
    return 0;
}

void wiimaker_audio_flush(void) {
    int i;
    for (i = 0; i < g_qn; i++)
        (void)wiimaker_audio_play(g_q[i].clip, g_q[i].volume);
    g_qn = 0;
}

int wiimaker_audio_load_wpack(const uint8_t *data, uint32_t size) {
    const uint8_t *p;
    const uint8_t *end;
    uint32_t tex_n, mesh_n, audio_n, i;

    wiimaker_audio_shutdown();
    if (!data || size < 16)
        return -1;
    p = data;
    end = data + size;
    if (memcmp(p, "WPACK001", 8) != 0)
        return -1;
    p += 8;
    tex_n = rd_u32(&p, end);
    mesh_n = rd_u32(&p, end);
    if (skip_textures(&p, end, tex_n) != 0)
        return -1;
    if (skip_meshes(&p, end, mesh_n) != 0)
        return -1;
    if (p + 4 > end) {
        /* Old pack: no audio TOC. */
        return 0;
    }
    audio_n = rd_u32(&p, end);
    for (i = 0; i < audio_n && p < end; i++) {
        char name[WIIMAKER_CLIP_NAME];
        uint32_t rate, nbytes;
        uint16_t channels;
        uint32_t padded;
        void *aligned;

        memset(name, 0, sizeof(name));
        if (read_str(&p, end, name, sizeof(name)) != 0)
            return -1;
        rate = rd_u32(&p, end);
        channels = rd_u16(&p, end);
        nbytes = rd_u32(&p, end);
        if (p + nbytes > end)
            return -1;

        if (g_clip_n < WIIMAKER_MAX_CLIPS && nbytes >= 2 && (channels == 1 || channels == 2) &&
            rate > 0) {
            padded = (nbytes + 31u) & ~31u;
            aligned = memalign(32, padded);
            if (aligned) {
                memset(aligned, 0, padded);
                memcpy(aligned, p, nbytes);
                byteswap16((uint16_t *)aligned, nbytes);
                DCFlushRange(aligned, padded);
                memcpy(g_clips[g_clip_n].name, name, WIIMAKER_CLIP_NAME);
                g_clips[g_clip_n].pcm = aligned;
                g_clips[g_clip_n].nbytes = nbytes;
                g_clips[g_clip_n].rate = rate;
                g_clips[g_clip_n].channels = channels;
                g_clip_n++;
            }
        }
        p += nbytes;
    }
    return 0;
}

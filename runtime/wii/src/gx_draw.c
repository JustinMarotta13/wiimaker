/*
 * Minimal GX helpers shared with the game via FFI.
 * Disc = untextured fan; Sprite = textured screen-space quad;
 * Text = untextured colored quads from the built-in 8x8 font bits.
 */

#include <gccore.h>
#include <malloc.h>
#include <math.h>
#include <string.h>

#include "font8x8.h"
#include "wiimaker_abi.h"

#define WIIMAKER_MAX_TEX 32

typedef struct {
    void *data;
    uint16_t width;
    uint16_t height;
    GXTexObj obj;
    int loaded;
} WiimakerTex;

static WiimakerTex g_tex[WIIMAKER_MAX_TEX];
static uint32_t g_tex_count;
static GXColor g_clear = {12, 18, 32, 255};

void wiimaker_gx_set_clear(uint8_t r, uint8_t g, uint8_t b, uint8_t a) {
    g_clear.r = r;
    g_clear.g = g;
    g_clear.b = b;
    g_clear.a = a;
    GX_SetCopyClear(g_clear, 0x00ffffff);
}

static void setup_untextured(void) {
    GX_ClearVtxDesc();
    GX_SetVtxDesc(GX_VA_POS, GX_DIRECT);
    GX_SetVtxDesc(GX_VA_CLR0, GX_DIRECT);
    GX_SetVtxAttrFmt(GX_VTXFMT0, GX_VA_POS, GX_POS_XYZ, GX_F32, 0);
    GX_SetVtxAttrFmt(GX_VTXFMT0, GX_VA_CLR0, GX_CLR_RGBA, GX_RGBA8, 0);
    GX_SetNumChans(1);
    GX_SetNumTexGens(0);
    GX_SetTevOrder(GX_TEVSTAGE0, GX_TEXCOORDNULL, GX_TEXMAP_NULL, GX_COLOR0A0);
    GX_SetTevOp(GX_TEVSTAGE0, GX_PASSCLR);
}

static void setup_textured(void) {
    GX_ClearVtxDesc();
    GX_SetVtxDesc(GX_VA_POS, GX_DIRECT);
    GX_SetVtxDesc(GX_VA_CLR0, GX_DIRECT);
    GX_SetVtxDesc(GX_VA_TEX0, GX_DIRECT);
    GX_SetVtxAttrFmt(GX_VTXFMT0, GX_VA_POS, GX_POS_XYZ, GX_F32, 0);
    GX_SetVtxAttrFmt(GX_VTXFMT0, GX_VA_CLR0, GX_CLR_RGBA, GX_RGBA8, 0);
    GX_SetVtxAttrFmt(GX_VTXFMT0, GX_VA_TEX0, GX_TEX_ST, GX_F32, 0);
    GX_SetNumChans(1);
    GX_SetNumTexGens(1);
    GX_SetTevOrder(GX_TEVSTAGE0, GX_TEXCOORD0, GX_TEXMAP0, GX_COLOR0A0);
    GX_SetTevOp(GX_TEVSTAGE0, GX_MODULATE);
}

void wiimaker_gx_draw_disc(float x, float y, float radius, uint32_t rgba8) {
    const int segments = 24;
    setup_untextured();
    GX_Begin(GX_TRIANGLEFAN, GX_VTXFMT0, segments + 2);
    GX_Position3f32(x, y, 0.0f);
    GX_Color1u32(rgba8);
    for (int i = 0; i <= segments; i++) {
        float a = (float)i / (float)segments * 6.2831853f;
        GX_Position3f32(x + cosf(a) * radius, y + sinf(a) * radius, 0.0f);
        GX_Color1u32(rgba8);
    }
    GX_End();
}

void wiimaker_gx_draw_sprite(uint32_t tex_id, float x, float y, float w, float h,
                             float u0, float v0, float u1, float v1, uint32_t rgba8) {
    if (tex_id >= g_tex_count || !g_tex[tex_id].loaded)
        return;

    setup_textured();
    GX_LoadTexObj(&g_tex[tex_id].obj, GX_TEXMAP0);

    float x0 = x;
    float y0 = y;
    float x1 = x + w;
    float y1 = y + h;

    GX_Begin(GX_QUADS, GX_VTXFMT0, 4);
    GX_Position3f32(x0, y0, 0.0f);
    GX_Color1u32(rgba8);
    GX_TexCoord2f32(u0, v0);

    GX_Position3f32(x1, y0, 0.0f);
    GX_Color1u32(rgba8);
    GX_TexCoord2f32(u1, v0);

    GX_Position3f32(x1, y1, 0.0f);
    GX_Color1u32(rgba8);
    GX_TexCoord2f32(u1, v1);

    GX_Position3f32(x0, y1, 0.0f);
    GX_Color1u32(rgba8);
    GX_TexCoord2f32(u0, v1);
    GX_End();
}

static unsigned char map_font_char(unsigned char ch) {
    if (ch >= FONT8X8_FIRST && ch <= FONT8X8_LAST)
        return ch;
    return FONT8X8_MISSING;
}

/* Advance one printable unit (ASCII or one '?' for a UTF-8 sequence). Newline → '\n'. */
static int next_text_unit(const char *s, uint16_t len, uint16_t *i, unsigned char *out) {
    if (*i >= len)
        return 0;
    unsigned char c = (unsigned char)s[*i];
    (*i)++;
    if (c == '\n') {
        *out = '\n';
        return 1;
    }
    if (c == '\r') {
        if (*i < len && (unsigned char)s[*i] == '\n')
            (*i)++;
        *out = '\n';
        return 1;
    }
    if (c < 0x80u) {
        *out = map_font_char(c);
        return 1;
    }
    int extra = 0;
    if ((c & 0xE0u) == 0xC0u)
        extra = 1;
    else if ((c & 0xF0u) == 0xE0u)
        extra = 2;
    else if ((c & 0xF8u) == 0xF0u)
        extra = 3;
    while (extra-- > 0 && *i < len && ((unsigned char)s[*i] & 0xC0u) == 0x80u)
        (*i)++;
    *out = FONT8X8_MISSING;
    return 1;
}

static int count_line_cols(const char *s, uint16_t len, uint16_t start) {
    int cols = 0;
    uint16_t i = start;
    unsigned char ch;
    while (next_text_unit(s, len, &i, &ch)) {
        if (ch == '\n')
            break;
        cols++;
    }
    return cols;
}

static float text_line_origin_x(float anchor_x, int cols, float glyph_w, uint8_t align) {
    float w = (float)cols * glyph_w;
    if (align == 1)
        return anchor_x - w * 0.5f;
    if (align == 2)
        return anchor_x - w;
    return anchor_x;
}

static void draw_glyph_quads(float x, float y, float cell, uint32_t rgba8, unsigned char ch) {
    const uint8_t *g = FONT8X8_GLYPHS[map_font_char(ch) - FONT8X8_FIRST];
    int n = 0;
    for (int py = 0; py < FONT8X8_CELL; py++) {
        uint8_t bits = g[py];
        for (int px = 0; px < FONT8X8_CELL; px++) {
            if ((bits >> px) & 1)
                n++;
        }
    }
    if (n == 0)
        return;

    GX_Begin(GX_QUADS, GX_VTXFMT0, (u16)(n * 4));
    for (int py = 0; py < FONT8X8_CELL; py++) {
        uint8_t bits = g[py];
        for (int px = 0; px < FONT8X8_CELL; px++) {
            if (((bits >> px) & 1) == 0)
                continue;
            float x0 = x + (float)px * cell;
            float y0 = y + (float)py * cell;
            float x1 = x0 + cell;
            float y1 = y0 + cell;
            GX_Position3f32(x0, y0, 0.0f);
            GX_Color1u32(rgba8);
            GX_Position3f32(x1, y0, 0.0f);
            GX_Color1u32(rgba8);
            GX_Position3f32(x1, y1, 0.0f);
            GX_Color1u32(rgba8);
            GX_Position3f32(x0, y1, 0.0f);
            GX_Color1u32(rgba8);
        }
    }
    GX_End();
}

void wiimaker_gx_draw_text(float x, float y, const char *text, uint16_t len, float size,
                           uint8_t align, uint32_t rgba8) {
    if (!text || size <= 0.5f)
        return;
    if (len == 0) {
        size_t n = strlen(text);
        if (n > 0xffffu)
            n = 0xffffu;
        len = (uint16_t)n;
    }
    float cell = size / (float)FONT8X8_CELL;
    setup_untextured();

    float cy = y;
    uint16_t i = 0;
    while (i < len) {
        uint16_t line_start = i;
        int cols = count_line_cols(text, len, line_start);
        float cx = text_line_origin_x(x, cols, size, align);
        unsigned char ch;
        while (next_text_unit(text, len, &i, &ch)) {
            if (ch == '\n')
                break;
            draw_glyph_quads(cx, cy, cell, rgba8, ch);
            cx += size;
        }
        cy += size;
    }
}

uint16_t wiimaker_tex_width(uint32_t tex_id) {
    if (tex_id >= g_tex_count)
        return 0;
    return g_tex[tex_id].width;
}

uint16_t wiimaker_tex_height(uint32_t tex_id) {
    if (tex_id >= g_tex_count)
        return 0;
    return g_tex[tex_id].height;
}

uint32_t wiimaker_tex_count(void) { return g_tex_count; }

void wiimaker_tex_shutdown(void) {
    for (uint32_t i = 0; i < g_tex_count; i++) {
        if (g_tex[i].data) {
            free(g_tex[i].data);
            g_tex[i].data = NULL;
        }
        g_tex[i].loaded = 0;
    }
    g_tex_count = 0;
}

/* ---- tiny LE readers for embedded .wpack ---- */

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

int wiimaker_tex_load_wpack(const uint8_t *data, uint32_t size) {
    wiimaker_tex_shutdown();
    if (!data || size < 16)
        return -1;
    const uint8_t *p = data;
    const uint8_t *end = data + size;
    if (memcmp(p, "WPACK001", 8) != 0)
        return -1;
    p += 8;
    uint32_t tex_n = rd_u32(&p, end);
    uint32_t mesh_n = rd_u32(&p, end);
    (void)mesh_n;
    if (tex_n > WIIMAKER_MAX_TEX)
        tex_n = WIIMAKER_MAX_TEX;

    for (uint32_t i = 0; i < tex_n; i++) {
        if (skip_str(&p, end) != 0)
            return -1;
        uint16_t w = rd_u16(&p, end);
        uint16_t h = rd_u16(&p, end);
        uint32_t len = rd_u32(&p, end);
        if (p + len > end || w == 0 || h == 0)
            return -1;

        void *aligned = memalign(32, len);
        if (!aligned)
            return -1;
        memcpy(aligned, p, len);
        DCFlushRange(aligned, len);
        p += len;

        GX_InitTexObj(&g_tex[i].obj, aligned, w, h, GX_TF_RGB5A3, GX_CLAMP, GX_CLAMP, GX_FALSE);
        GX_InitTexObjLOD(&g_tex[i].obj, GX_NEAR, GX_NEAR, 0.0f, 0.0f, 0.0f, GX_DISABLE, GX_DISABLE,
                         GX_ANISO_1);
        g_tex[i].data = aligned;
        g_tex[i].width = w;
        g_tex[i].height = h;
        g_tex[i].loaded = 1;
        g_tex_count++;
    }
    return 0;
}

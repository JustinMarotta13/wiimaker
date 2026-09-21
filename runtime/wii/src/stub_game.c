/*
 * Scene-driven C game for Wii (until Rust staticlib lands).
 * Loads embedded assets.wpack + scene.wscn, draws sprites/discs/text/tilemaps,
 * and keeps hello-orb Player / OrbShadow gameplay.
 */

#include "wiimaker_abi.h"

#include <math.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

/* Linked via powerpc-eabi-objcopy -I binary (see Makefile). */
extern const uint8_t _binary_assets_wpack_start[];
extern const uint8_t _binary_assets_wpack_end[];
extern const uint8_t _binary_scene_wscn_start[];
extern const uint8_t _binary_scene_wscn_end[];

#define KIND_NONE 0
#define KIND_SPRITE 1
#define KIND_DISC 2
#define KIND_TILEMAP 3
#define KIND_TEXT 4
#define MAX_ENTITIES 64
#define MAX_NAME 48
#define MAX_TEXT 256
#define TILE_NO_TEX 0xFFFFu
#define TILE_AUTO_OFF 0
#define TILE_AUTO_ID 1
#define TILE_AUTO_SOLID 2
#define MAX_TILE_PAL 32
#define MAX_TILE_FRAMES 16
#define MAX_TILE_CELLS 16384

typedef struct {
    uint16_t id;
    uint8_t color[4];
    uint16_t tex;
    float u0, v0, u1, v1;
    uint8_t auto_mode;
    uint16_t auto_tex[16];
    float auto_u0[16], auto_v0[16], auto_u1[16], auto_v1[16];
    uint8_t frame_n;
    float fps;
    float time;
    uint16_t frame_tex[MAX_TILE_FRAMES];
    float frame_u0[MAX_TILE_FRAMES];
    float frame_v0[MAX_TILE_FRAMES];
    float frame_u1[MAX_TILE_FRAMES];
    float frame_v1[MAX_TILE_FRAMES];
} TilePal;

typedef struct {
    float cell, ox, oy;
    uint16_t w, h;
    uint32_t n;
    uint16_t *cells;
    uint8_t *solid;
    uint16_t pal_n;
    TilePal pal[MAX_TILE_PAL];
} WiiTilemap;

typedef struct {
    char name[MAX_NAME];
    float x, y;
    float sx, sy;
    uint8_t kind;
    uint16_t tex;
    float size_w, size_h;
    float u0, v0, u1, v1;
    float pivot_x, pivot_y;
    float radius;
    uint8_t color[4];
    float z;
    char text[MAX_TEXT];
    uint16_t text_len;
    float text_size;
    uint8_t text_align;
    WiiTilemap *tm;
} Entity;

static Entity ents[MAX_ENTITIES];
static int ent_count;
static int player_i = -1;
static int shadow_i = -1;
static float base_radius = 36.0f;
static float pulse = 0.0f;
static float hue = 0.0f;
static float screen_w = 640.0f;
static float screen_h = 480.0f;
static int a_was_down = 0;

static float clampf(float v, float lo, float hi) {
    if (v < lo)
        return lo;
    if (v > hi)
        return hi;
    return v;
}

static float lerpf(float a, float b, float t) { return a + (b - a) * t; }

static uint32_t rgba_pack(const uint8_t c[4]) {
    return ((uint32_t)c[0] << 24) | ((uint32_t)c[1] << 16) | ((uint32_t)c[2] << 8) | (uint32_t)c[3];
}

static uint32_t orb_rgba(float phase, float pulse_amt) {
    float t = sinf(phase * 6.2831853f) * 0.5f + 0.5f;
    float r = lerpf(72.0f, 255.0f, t) + pulse_amt * 40.0f;
    float g = lerpf(210.0f, 96.0f, t) + pulse_amt * 16.0f;
    float b = lerpf(160.0f, 88.0f, t) + pulse_amt * 8.0f;
    if (r > 255.0f)
        r = 255.0f;
    if (g > 255.0f)
        g = 255.0f;
    if (b > 255.0f)
        b = 255.0f;
    return ((uint32_t)r << 24) | ((uint32_t)g << 16) | ((uint32_t)b << 8) | 0xffu;
}

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

static float rd_f32(const uint8_t **p, const uint8_t *end) {
    uint32_t bits = rd_u32(p, end);
    float f;
    memcpy(&f, &bits, 4);
    return f;
}

static void free_tilemap(WiiTilemap *tm) {
    if (!tm)
        return;
    free(tm->cells);
    free(tm->solid);
    free(tm);
}

static void free_all_tilemaps(void) {
    for (int i = 0; i < ent_count; i++) {
        free_tilemap(ents[i].tm);
        ents[i].tm = NULL;
    }
}

static int tm_in_bounds(const WiiTilemap *tm, int x, int y) {
    return tm && x >= 0 && y >= 0 && x < (int)tm->w && y < (int)tm->h;
}

static uint16_t tm_cell(const WiiTilemap *tm, int x, int y) {
    if (!tm_in_bounds(tm, x, y) || !tm->cells)
        return 0;
    return tm->cells[(uint32_t)y * tm->w + (uint32_t)x];
}

static int tm_solid_at(const WiiTilemap *tm, int x, int y) {
    if (!tm_in_bounds(tm, x, y) || !tm->solid)
        return 0;
    uint32_t i = (uint32_t)y * tm->w + (uint32_t)x;
    return (tm->solid[i / 8] >> (i % 8)) & 1;
}

static uint8_t tm_autotile_mask(const WiiTilemap *tm, int x, int y, uint16_t id, uint8_t mode) {
    int n, e, s, w;
    if (mode == TILE_AUTO_SOLID) {
        n = !tm_in_bounds(tm, x, y - 1) || tm_solid_at(tm, x, y - 1);
        e = !tm_in_bounds(tm, x + 1, y) || tm_solid_at(tm, x + 1, y);
        s = !tm_in_bounds(tm, x, y + 1) || tm_solid_at(tm, x, y + 1);
        w = !tm_in_bounds(tm, x - 1, y) || tm_solid_at(tm, x - 1, y);
    } else {
        n = tm_in_bounds(tm, x, y - 1) && id != 0 && tm_cell(tm, x, y - 1) == id;
        e = tm_in_bounds(tm, x + 1, y) && id != 0 && tm_cell(tm, x + 1, y) == id;
        s = tm_in_bounds(tm, x, y + 1) && id != 0 && tm_cell(tm, x, y + 1) == id;
        w = tm_in_bounds(tm, x - 1, y) && id != 0 && tm_cell(tm, x - 1, y) == id;
    }
    return (uint8_t)((n ? 1 : 0) | (e ? 2 : 0) | (s ? 4 : 0) | (w ? 8 : 0));
}

static TilePal *tm_pal_for(WiiTilemap *tm, uint16_t id) {
    if (!tm)
        return NULL;
    for (uint16_t i = 0; i < tm->pal_n; i++) {
        if (tm->pal[i].id == id)
            return &tm->pal[i];
    }
    if (tm->pal_n > 0)
        return &tm->pal[0];
    return NULL;
}

static void tm_tick(WiiTilemap *tm, float dt) {
    if (!tm)
        return;
    for (uint16_t i = 0; i < tm->pal_n; i++) {
        TilePal *pal = &tm->pal[i];
        if (pal->frame_n <= 1 || pal->fps <= 0.0f)
            continue;
        pal->time += dt;
        float dur = 1.0f / pal->fps;
        int n = pal->frame_n;
        int idx = (int)(pal->time / dur);
        if (n > 0)
            idx %= n;
        if (idx < 0)
            idx = 0;
        float cycle = dur * (float)n;
        if (cycle > 0.0f && pal->time >= cycle)
            pal->time = fmodf(pal->time, cycle);
        pal->tex = pal->frame_tex[idx];
        pal->u0 = pal->frame_u0[idx];
        pal->v0 = pal->frame_v0[idx];
        pal->u1 = pal->frame_u1[idx];
        pal->v1 = pal->frame_v1[idx];
    }
}

static void tm_draw(const Entity *e) {
    WiiTilemap *tm = e->tm;
    if (!tm || !tm->cells || tm->w == 0 || tm->h == 0)
        return;
    float cell_w = tm->cell * e->sx;
    float cell_h = tm->cell * e->sy;
    if (fabsf(cell_w) < 1e-6f || fabsf(cell_h) < 1e-6f)
        return;
    float ox = e->x + tm->ox * e->sx;
    float oy = e->y + tm->oy * e->sy;
    static const uint8_t k_default[4] = {48, 88, 176, 255};

    for (int y = 0; y < (int)tm->h; y++) {
        for (int x = 0; x < (int)tm->w; x++) {
            uint16_t id = tm_cell(tm, x, y);
            if (id == 0)
                continue;
            TilePal *pal = tm_pal_for(tm, id);
            const uint8_t *col = pal ? pal->color : k_default;
            uint16_t tex = pal ? pal->tex : TILE_NO_TEX;
            float u0 = pal ? pal->u0 : 0.0f;
            float v0 = pal ? pal->v0 : 0.0f;
            float u1 = pal ? pal->u1 : 1.0f;
            float v1 = pal ? pal->v1 : 1.0f;
            if (pal && pal->auto_mode != TILE_AUTO_OFF) {
                uint8_t mask = tm_autotile_mask(tm, x, y, id, pal->auto_mode);
                if (pal->auto_tex[mask] != TILE_NO_TEX) {
                    tex = pal->auto_tex[mask];
                    u0 = pal->auto_u0[mask];
                    v0 = pal->auto_v0[mask];
                    u1 = pal->auto_u1[mask];
                    v1 = pal->auto_v1[mask];
                }
            }
            float dx = ox + (float)x * cell_w;
            float dy = oy + (float)y * cell_h;
            uint32_t rgba = rgba_pack(col);
            if (tex != TILE_NO_TEX && tex < wiimaker_tex_count()) {
                wiimaker_gx_draw_sprite(tex, dx, dy, cell_w, cell_h, u0, v0, u1, v1, rgba);
            } else {
                wiimaker_gx_draw_quad(dx, dy, cell_w, cell_h, rgba);
            }
        }
    }
}

static int load_tilemap_payload(Entity *e, const uint8_t *tp, const uint8_t *tend) {
    WiiTilemap *tm = (WiiTilemap *)calloc(1, sizeof(WiiTilemap));
    if (!tm)
        return -1;
    tm->cell = rd_f32(&tp, tend);
    tm->ox = rd_f32(&tp, tend);
    tm->oy = rd_f32(&tp, tend);
    tm->w = rd_u16(&tp, tend);
    tm->h = rd_u16(&tp, tend);
    e->z = rd_f32(&tp, tend);
    tm->n = rd_u32(&tp, tend);
    uint32_t expect = (uint32_t)tm->w * (uint32_t)tm->h;
    if (tm->n != expect || tm->n > MAX_TILE_CELLS) {
        free(tm);
        e->kind = KIND_NONE;
        return 0;
    }
    if (tm->n > 0) {
        tm->cells = (uint16_t *)malloc(tm->n * sizeof(uint16_t));
        if (!tm->cells) {
            free(tm);
            return -1;
        }
        for (uint32_t ci = 0; ci < tm->n; ci++)
            tm->cells[ci] = rd_u16(&tp, tend);
    }
    uint32_t sbytes = (tm->n + 7u) / 8u;
    if (sbytes == 0)
        sbytes = 1;
    tm->solid = (uint8_t *)calloc(sbytes, 1);
    if (!tm->solid) {
        free(tm->cells);
        free(tm);
        return -1;
    }
    if (tp + ((tm->n + 7u) / 8u) <= tend) {
        uint32_t nsolid = (tm->n + 7u) / 8u;
        memcpy(tm->solid, tp, nsolid);
        tp += nsolid;
    }

    if (tp + 2 <= tend) {
        uint16_t pal_n = rd_u16(&tp, tend);
        uint16_t want = pal_n;
        if (want > MAX_TILE_PAL)
            want = MAX_TILE_PAL;
        for (uint16_t pi = 0; pi < pal_n && tp < tend; pi++) {
            TilePal scratch;
            memset(&scratch, 0, sizeof(scratch));
            int m;
            for (m = 0; m < 16; m++)
                scratch.auto_tex[m] = TILE_NO_TEX;
            scratch.id = rd_u16(&tp, tend);
            if (tp + 4 > tend)
                break;
            memcpy(scratch.color, tp, 4);
            tp += 4;
            scratch.tex = rd_u16(&tp, tend);
            scratch.u0 = rd_f32(&tp, tend);
            scratch.v0 = rd_f32(&tp, tend);
            scratch.u1 = rd_f32(&tp, tend);
            scratch.v1 = rd_f32(&tp, tend);
            scratch.auto_mode = (tp < tend) ? *tp++ : 0;
            uint16_t auto_bits = rd_u16(&tp, tend);
            for (m = 0; m < 16; m++) {
                if (auto_bits & (1u << m)) {
                    scratch.auto_tex[m] = rd_u16(&tp, tend);
                    scratch.auto_u0[m] = rd_f32(&tp, tend);
                    scratch.auto_v0[m] = rd_f32(&tp, tend);
                    scratch.auto_u1[m] = rd_f32(&tp, tend);
                    scratch.auto_v1[m] = rd_f32(&tp, tend);
                }
            }
            scratch.frame_n = (tp < tend) ? *tp++ : 0;
            scratch.fps = rd_f32(&tp, tend);
            if (scratch.frame_n > MAX_TILE_FRAMES)
                scratch.frame_n = MAX_TILE_FRAMES;
            for (uint8_t f = 0; f < scratch.frame_n; f++) {
                scratch.frame_tex[f] = rd_u16(&tp, tend);
                scratch.frame_u0[f] = rd_f32(&tp, tend);
                scratch.frame_v0[f] = rd_f32(&tp, tend);
                scratch.frame_u1[f] = rd_f32(&tp, tend);
                scratch.frame_v1[f] = rd_f32(&tp, tend);
            }
            if (pi < want) {
                tm->pal[tm->pal_n] = scratch;
                tm->pal_n++;
            }
        }
    }
    e->tm = tm;
    return 0;
}

static int load_scene(const uint8_t *data, uint32_t size) {
    free_all_tilemaps();
    ent_count = 0;
    player_i = -1;
    shadow_i = -1;
    if (!data || size < 16)
        return -1;
    const uint8_t *p = data;
    const uint8_t *end = data + size;
    if (memcmp(p, "WSCN0003", 8) != 0 && memcmp(p, "WSCN0002", 8) != 0)
        return -1;
    p += 8;
    uint8_t clear[4];
    memcpy(clear, p, 4);
    p += 4;
    wiimaker_gx_set_clear(clear[0], clear[1], clear[2], clear[3]);

    uint32_t n = rd_u32(&p, end);
    if (n > MAX_ENTITIES)
        n = MAX_ENTITIES;

    for (uint32_t i = 0; i < n; i++) {
        Entity *e = &ents[ent_count];
        memset(e, 0, sizeof(*e));
        uint16_t name_len = rd_u16(&p, end);
        if (p + name_len > end)
            return -1;
        uint16_t copy = name_len < (MAX_NAME - 1) ? name_len : (MAX_NAME - 1);
        memcpy(e->name, p, copy);
        e->name[copy] = '\0';
        p += name_len;

        e->x = rd_f32(&p, end);
        e->y = rd_f32(&p, end);
        (void)rd_f32(&p, end); /* tz */
        e->sx = rd_f32(&p, end);
        e->sy = rd_f32(&p, end);
        (void)rd_f32(&p, end); /* sz */

        e->kind = (p < end) ? *p++ : KIND_NONE;
        if (e->kind == KIND_SPRITE) {
            e->tex = rd_u16(&p, end);
            e->size_w = rd_f32(&p, end);
            e->size_h = rd_f32(&p, end);
            e->u0 = rd_f32(&p, end);
            e->v0 = rd_f32(&p, end);
            e->u1 = rd_f32(&p, end);
            e->v1 = rd_f32(&p, end);
            e->pivot_x = rd_f32(&p, end);
            e->pivot_y = rd_f32(&p, end);
            if (p + 4 > end)
                return -1;
            memcpy(e->color, p, 4);
            p += 4;
            e->z = rd_f32(&p, end);
            /* Legacy fallback: full UV → content UV for PoT pad. */
            if (e->u0 == 0.0f && e->v0 == 0.0f && e->u1 == 1.0f && e->v1 == 1.0f) {
                float tw = (float)wiimaker_tex_width(e->tex);
                float th = (float)wiimaker_tex_height(e->tex);
                if (tw > 0.0f && th > 0.0f) {
                    e->u1 = e->size_w / tw;
                    e->v1 = e->size_h / th;
                    if (e->u1 > 1.0f)
                        e->u1 = 1.0f;
                    if (e->v1 > 1.0f)
                        e->v1 = 1.0f;
                }
            }
        } else if (e->kind == KIND_DISC) {
            e->radius = rd_f32(&p, end);
            if (p + 4 > end)
                return -1;
            memcpy(e->color, p, 4);
            p += 4;
            e->z = rd_f32(&p, end);
        } else if (e->kind == KIND_TILEMAP) {
            uint32_t plen = rd_u32(&p, end);
            if (p + plen > end)
                return -1;
            if (load_tilemap_payload(e, p, p + plen) != 0)
                return -1;
            p += plen;
        } else if (e->kind == KIND_TEXT) {
            /* u16 len + utf8, f32 size, u8 align, u8[4] rgba, f32 z */
            uint16_t slen = rd_u16(&p, end);
            if (p + slen > end)
                return -1;
            uint16_t tcopy = slen < (MAX_TEXT - 1) ? slen : (MAX_TEXT - 1);
            memcpy(e->text, p, tcopy);
            e->text[tcopy] = '\0';
            e->text_len = tcopy;
            p += slen;
            e->text_size = rd_f32(&p, end);
            e->text_align = (p < end) ? *p++ : 0;
            if (p + 4 > end)
                return -1;
            memcpy(e->color, p, 4);
            p += 4;
            e->z = rd_f32(&p, end);
        }

        if (strcmp(e->name, "Player") == 0) {
            player_i = ent_count;
            base_radius = e->radius > 0.0f ? e->radius : 36.0f;
        } else if (strcmp(e->name, "OrbShadow") == 0) {
            shadow_i = ent_count;
        }
        ent_count++;
    }
    return 0;
}

void wiimaker_game_init(uint32_t fb_w, uint32_t fb_h) {
    screen_w = (float)fb_w;
    screen_h = (float)fb_h;
    pulse = 0.0f;
    hue = 0.0f;
    a_was_down = 0;

    uint32_t wpack_size =
        (uint32_t)(_binary_assets_wpack_end - _binary_assets_wpack_start);
    uint32_t wscn_size = (uint32_t)(_binary_scene_wscn_end - _binary_scene_wscn_start);
    wiimaker_tex_load_wpack(_binary_assets_wpack_start, wpack_size);
    load_scene(_binary_scene_wscn_start, wscn_size);
}

int wiimaker_game_frame(const WiimakerInput *input, float dt) {
    float mx = input->main_x;
    float my = input->main_y;

    if (fabsf(mx) < 0.15f && fabsf(my) < 0.15f) {
        mx = 0.0f;
        my = 0.0f;
        if (input->buttons & WIIMAKER_BTN_LEFT)
            mx -= 1.0f;
        if (input->buttons & WIIMAKER_BTN_RIGHT)
            mx += 1.0f;
        if (input->buttons & WIIMAKER_BTN_UP)
            my += 1.0f;
        if (input->buttons & WIIMAKER_BTN_DOWN)
            my -= 1.0f;
    }

    if (player_i >= 0) {
        Entity *player = &ents[player_i];
        float r = base_radius * (1.0f + pulse * 0.35f);
        float speed = 220.0f * dt;
        player->x = clampf(player->x + mx * speed, r, screen_w - r);
        player->y = clampf(player->y - my * speed, r, screen_h - r);
        if (shadow_i >= 0) {
            ents[shadow_i].x = player->x + 4.0f;
            ents[shadow_i].y = player->y + 6.0f;
        }
    }

    int a_down = (input->buttons & WIIMAKER_BTN_A) != 0;
    if (a_down && !a_was_down)
        pulse = 1.0f;
    a_was_down = a_down;
    pulse = clampf(pulse - dt * 2.5f, 0.0f, 1.0f);

    hue += dt * 0.35f;
    if (hue > 1.0f)
        hue -= 1.0f;

    if (player_i >= 0)
        ents[player_i].radius = base_radius * (1.0f + pulse * 0.35f);

    for (int i = 0; i < ent_count; i++) {
        if (ents[i].kind == KIND_TILEMAP)
            tm_tick(ents[i].tm, dt);
    }

    int order[MAX_ENTITIES];
    for (int i = 0; i < ent_count; i++)
        order[i] = i;
    for (int i = 1; i < ent_count; i++) {
        int key = order[i];
        int j = i - 1;
        while (j >= 0 && ents[order[j]].z > ents[key].z) {
            order[j + 1] = order[j];
            j--;
        }
        order[j + 1] = key;
    }

    for (int oi = 0; oi < ent_count; oi++) {
        Entity *e = &ents[order[oi]];
        if (e->kind == KIND_SPRITE) {
            float dw = e->size_w * e->sx;
            float dh = e->size_h * e->sy;
            float x = e->x - dw * e->pivot_x;
            float y = e->y - dh * e->pivot_y;
            wiimaker_gx_draw_sprite(e->tex, x, y, dw, dh, e->u0, e->v0, e->u1, e->v1,
                                    rgba_pack(e->color));
        } else if (e->kind == KIND_DISC) {
            uint32_t col = rgba_pack(e->color);
            if (player_i >= 0 && e == &ents[player_i])
                col = orb_rgba(hue, pulse);
            float scale = e->sx > e->sy ? e->sx : e->sy;
            wiimaker_gx_draw_disc(e->x, e->y, e->radius * scale, col);
        } else if (e->kind == KIND_TILEMAP) {
            tm_draw(e);
        } else if (e->kind == KIND_TEXT) {
            float scale = e->sx > e->sy ? e->sx : e->sy;
            if (scale < 0.0f)
                scale = -scale;
            wiimaker_gx_draw_text(e->x, e->y, e->text, e->text_len, e->text_size * scale,
                                  e->text_align, rgba_pack(e->color));
        }
    }

    if (input->buttons & WIIMAKER_BTN_A)
        wiimaker_gx_draw_disc(24.0f, 24.0f, 8.0f, 0xff6058ffu);

    if (input->buttons & WIIMAKER_BTN_START)
        return 1;
    return 0;
}

void wiimaker_game_shutdown(void) {
    free_all_tilemaps();
    ent_count = 0;
    wiimaker_tex_shutdown();
}

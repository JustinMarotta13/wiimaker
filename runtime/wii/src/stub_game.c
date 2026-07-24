/*
 * Scene-driven C game for Wii (until Rust staticlib lands).
 * Loads embedded assets.wpack + scene.wscn, draws sprites/discs,
 * and keeps hello-orb Player / OrbShadow gameplay.
 */

#include "wiimaker_abi.h"

#include <math.h>
#include <stdint.h>
#include <string.h>

/* Linked via powerpc-eabi-objcopy -I binary (see Makefile). */
extern const uint8_t _binary_assets_wpack_start[];
extern const uint8_t _binary_assets_wpack_end[];
extern const uint8_t _binary_scene_wscn_start[];
extern const uint8_t _binary_scene_wscn_end[];

#define KIND_NONE 0
#define KIND_SPRITE 1
#define KIND_DISC 2
#define MAX_ENTITIES 64
#define MAX_NAME 48

typedef struct {
    char name[MAX_NAME];
    float x, y;
    float sx, sy;
    uint8_t kind;
    uint16_t tex;
    float size_w, size_h;
    float radius;
    uint8_t color[4];
    float z;
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

static int load_scene(const uint8_t *data, uint32_t size) {
    ent_count = 0;
    player_i = -1;
    shadow_i = -1;
    if (!data || size < 16)
        return -1;
    const uint8_t *p = data;
    const uint8_t *end = data + size;
    if (memcmp(p, "WSCN0001", 8) != 0)
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
            if (p + 4 > end)
                return -1;
            memcpy(e->color, p, 4);
            p += 4;
            e->z = rd_f32(&p, end);
        } else if (e->kind == KIND_DISC) {
            e->radius = rd_f32(&p, end);
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
            float x = e->x - dw * 0.5f;
            float y = e->y - dh * 0.5f;
            float tw = (float)wiimaker_tex_width(e->tex);
            float th = (float)wiimaker_tex_height(e->tex);
            float u1 = 1.0f;
            float v1 = 1.0f;
            /* Content-sized sprites on PoT-padded textures sample the top-left. */
            if (tw > 0.0f && th > 0.0f) {
                u1 = e->size_w / tw;
                v1 = e->size_h / th;
                if (u1 > 1.0f)
                    u1 = 1.0f;
                if (v1 > 1.0f)
                    v1 = 1.0f;
            }
            wiimaker_gx_draw_sprite(e->tex, x, y, dw, dh, 0.0f, 0.0f, u1, v1, rgba_pack(e->color));
        } else if (e->kind == KIND_DISC) {
            uint32_t col = rgba_pack(e->color);
            if (player_i >= 0 && e == &ents[player_i])
                col = orb_rgba(hue, pulse);
            float scale = e->sx > e->sy ? e->sx : e->sy;
            wiimaker_gx_draw_disc(e->x, e->y, e->radius * scale, col);
        }
    }

    if (input->buttons & WIIMAKER_BTN_A)
        wiimaker_gx_draw_disc(24.0f, 24.0f, 8.0f, 0xff6058ffu);

    if (input->buttons & WIIMAKER_BTN_START)
        return 1;
    return 0;
}

void wiimaker_game_shutdown(void) { wiimaker_tex_shutdown(); }

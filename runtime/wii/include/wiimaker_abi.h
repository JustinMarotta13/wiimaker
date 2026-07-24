#ifndef WIIMAKER_ABI_H
#define WIIMAKER_ABI_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define WIIMAKER_BTN_A (1u << 0)
#define WIIMAKER_BTN_B (1u << 1)
#define WIIMAKER_BTN_X (1u << 2)
#define WIIMAKER_BTN_Y (1u << 3)
#define WIIMAKER_BTN_START (1u << 4)
#define WIIMAKER_BTN_Z (1u << 5)
#define WIIMAKER_BTN_L (1u << 6)
#define WIIMAKER_BTN_R (1u << 7)
#define WIIMAKER_BTN_UP (1u << 8)
#define WIIMAKER_BTN_DOWN (1u << 9)
#define WIIMAKER_BTN_LEFT (1u << 10)
#define WIIMAKER_BTN_RIGHT (1u << 11)

typedef struct WiimakerInput {
    float main_x;
    float main_y;
    float c_x;
    float c_y;
    uint32_t buttons;
} WiimakerInput;

/* Implemented by the game staticlib (Rust) or C scene player. */
void wiimaker_game_init(uint32_t fb_w, uint32_t fb_h);
int wiimaker_game_frame(const WiimakerInput *input, float dt);
void wiimaker_game_shutdown(void);

/* GX helpers owned by runtime/wii (C). */
void wiimaker_gx_set_clear(uint8_t r, uint8_t g, uint8_t b, uint8_t a);
void wiimaker_gx_draw_disc(float x, float y, float radius, uint32_t rgba8);
void wiimaker_gx_draw_sprite(uint32_t tex_id, float x, float y, float w, float h,
                             float u0, float v0, float u1, float v1, uint32_t rgba8);

/* Texture upload from cooked `.wpack` (GX-tiled RGB5A3). */
int wiimaker_tex_load_wpack(const uint8_t *data, uint32_t size);
uint16_t wiimaker_tex_width(uint32_t tex_id);
uint16_t wiimaker_tex_height(uint32_t tex_id);
uint32_t wiimaker_tex_count(void);
void wiimaker_tex_shutdown(void);

#ifdef __cplusplus
}
#endif

#endif /* WIIMAKER_ABI_H */

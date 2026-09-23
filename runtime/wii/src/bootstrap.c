/*
 * wiimaker Wii bootstrap
 *
 * Owns VI / GX / PAD. Rust game code is linked as a staticlib and called each
 * frame via wiimaker_game_frame(). This avoids the pure-Rust Video::configure
 * heap-leak crash from the first wiimaker attempt.
 *
 * Build: see Makefile (devkitPro) or ../../tools/wii-build.sh (Docker).
 */

#include <asndlib.h>
#include <gccore.h>
#include <malloc.h>
#include <ogc/lwp_watchdog.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wiiuse/wpad.h>

#include "wiimaker_abi.h"

#define DEFAULT_FIFO_SIZE (256 * 1024)

static void *xfb[2];
static int fbi;
static GXRModeObj *rmode;
static bool running = true;

static void reset_cb(u32 irq, void *ctx) {
    (void)irq;
    (void)ctx;
    running = false;
}

static void power_cb(void) { running = false; }

static void init_video(void) {
    VIDEO_Init();
    WPAD_Init();
    /* Expansion analog (Classic ljs/rjs, Nunchuk js) needs more than buttons. */
    WPAD_SetDataFormat(WPAD_CHAN_ALL, WPAD_FMT_BTNS_ACC_IR);
    PAD_Init();

    rmode = VIDEO_GetPreferredMode(NULL);
    xfb[0] = MEM_K0_TO_K1(SYS_AllocateFramebuffer(rmode));
    xfb[1] = MEM_K0_TO_K1(SYS_AllocateFramebuffer(rmode));
    CON_Init(xfb[0], 20, 20, rmode->fbWidth, rmode->xfbHeight,
             rmode->fbWidth * VI_DISPLAY_PIX_SZ);
    VIDEO_Configure(rmode);
    VIDEO_SetNextFramebuffer(xfb[0]);
    VIDEO_SetBlack(FALSE);
    VIDEO_Flush();
    VIDEO_WaitVSync();
    if (rmode->viTVMode & VI_NON_INTERLACE)
        VIDEO_WaitVSync();

    void *fifo = MEM_K0_TO_K1(memalign(32, DEFAULT_FIFO_SIZE));
    memset(fifo, 0, DEFAULT_FIFO_SIZE);
    GX_Init(fifo, DEFAULT_FIFO_SIZE);
    GX_SetViewport(0, 0, rmode->fbWidth, rmode->efbHeight, 0, 1);
    GX_SetDispCopyYScale((f32)rmode->xfbHeight / (f32)rmode->efbHeight);
    GX_SetScissor(0, 0, rmode->fbWidth, rmode->efbHeight);
    GX_SetDispCopySrc(0, 0, rmode->fbWidth, rmode->efbHeight);
    GX_SetDispCopyDst(rmode->fbWidth, rmode->xfbHeight);
    GX_SetCopyClear((GXColor){12, 18, 32, 255}, 0x00ffffff);
    GX_SetCopyFilter(rmode->aa, rmode->sample_pattern, GX_TRUE, rmode->vfilter);
    GX_SetFieldMode(rmode->field_rendering,
                    ((rmode->viHeight == 2 * rmode->xfbHeight) ? GX_ENABLE
                                                               : GX_DISABLE));
    GX_SetCullMode(GX_CULL_NONE);
    GX_CopyDisp(xfb[fbi], GX_TRUE);
    GX_SetDispCopyGamma(GX_GM_1_0);

    /* PCM16 oneshots: host plays assets/*.wav; Wii plays the .wpack audio TOC. */
    ASND_Init();
    ASND_Pause(0);

    SYS_SetResetCallback(reset_cb);
    SYS_SetPowerCallback(power_cb);
}

/* Same idle zone as wiimaker-core `STICK_IDLE_DEADZONE`. */
#define STICK_IDLE 0.20f

static int stick_idle(f32 x, f32 y) {
    return (x * x + y * y) < (STICK_IDLE * STICK_IDLE);
}

static f32 clampf(f32 v, f32 lo, f32 hi) {
    if (v < lo)
        return lo;
    if (v > hi)
        return hi;
    return v;
}

/* joystick_t pos/center/min/max — ang 0 = up after calc_joystick_state. */
static f32 joy_axis(u8 pos, u8 center, u8 maxv, u8 minv) {
    f32 p = (f32)pos;
    f32 c = (f32)center;
    f32 mx = (f32)maxv;
    f32 mn = (f32)minv;
    if (mx <= mn) {
        c = 128.0f;
        mx = 255.0f;
        mn = 0.0f;
    }
    f32 range = (p >= c) ? (mx - c) : (c - mn);
    if (range < 1.0f)
        range = 1.0f;
    return clampf((p - c) / range, -1.0f, 1.0f);
}

static void joy_xy(const struct joystick_t *js, f32 *x, f32 *y) {
    *x = joy_axis(js->pos.x, js->center.x, js->max.x, js->min.x);
    *y = joy_axis(js->pos.y, js->center.y, js->max.y, js->min.y);
}

static void fill_input(WiimakerInput *out) {
    memset(out, 0, sizeof(*out));
    PAD_ScanPads();
    WPAD_ScanPads();

    u16 gcn = PAD_ButtonsHeld(0);
    out->main_x = (f32)PAD_StickX(0) / 128.0f;
    out->main_y = (f32)PAD_StickY(0) / 128.0f;
    out->c_x = (f32)PAD_SubStickX(0) / 128.0f;
    out->c_y = (f32)PAD_SubStickY(0) / 128.0f;

    if (gcn & PAD_BUTTON_A)
        out->buttons |= WIIMAKER_BTN_A;
    if (gcn & PAD_BUTTON_B)
        out->buttons |= WIIMAKER_BTN_B;
    if (gcn & PAD_BUTTON_X)
        out->buttons |= WIIMAKER_BTN_X;
    if (gcn & PAD_BUTTON_Y)
        out->buttons |= WIIMAKER_BTN_Y;
    if (gcn & PAD_BUTTON_START)
        out->buttons |= WIIMAKER_BTN_START;
    if (gcn & PAD_TRIGGER_Z)
        out->buttons |= WIIMAKER_BTN_Z;
    if (gcn & PAD_BUTTON_UP)
        out->buttons |= WIIMAKER_BTN_UP;
    if (gcn & PAD_BUTTON_DOWN)
        out->buttons |= WIIMAKER_BTN_DOWN;
    if (gcn & PAD_BUTTON_LEFT)
        out->buttons |= WIIMAKER_BTN_LEFT;
    if (gcn & PAD_BUTTON_RIGHT)
        out->buttons |= WIIMAKER_BTN_RIGHT;

    /* Core Wiimote: keep A/B/Plus/D-pad; 1→X, 2→Y, Minus→Z (common homebrew). */
    u32 wii = WPAD_ButtonsHeld(0);
    if (wii & WPAD_BUTTON_A)
        out->buttons |= WIIMAKER_BTN_A;
    if (wii & WPAD_BUTTON_B)
        out->buttons |= WIIMAKER_BTN_B;
    if (wii & WPAD_BUTTON_1)
        out->buttons |= WIIMAKER_BTN_X;
    if (wii & WPAD_BUTTON_2)
        out->buttons |= WIIMAKER_BTN_Y;
    if (wii & WPAD_BUTTON_PLUS)
        out->buttons |= WIIMAKER_BTN_START;
    if (wii & WPAD_BUTTON_HOME)
        out->buttons |= WIIMAKER_BTN_START;
    if (wii & WPAD_BUTTON_MINUS)
        out->buttons |= WIIMAKER_BTN_Z;
    if (wii & WPAD_BUTTON_UP)
        out->buttons |= WIIMAKER_BTN_UP;
    if (wii & WPAD_BUTTON_DOWN)
        out->buttons |= WIIMAKER_BTN_DOWN;
    if (wii & WPAD_BUTTON_LEFT)
        out->buttons |= WIIMAKER_BTN_LEFT;
    if (wii & WPAD_BUTTON_RIGHT)
        out->buttons |= WIIMAKER_BTN_RIGHT;

    /* Expansion high-word bits collide: Nunchuk Z == Classic UP, C == LEFT
     * (`wpad.h`). Probe first; OR Classic digital only when EXP_CLASSIC.
     * EXP_NONE / EXP_NUNCHUK must not treat those bits as D-pad (idle analog
     * would synthesize +Y and GridMover prefers D-pad over stick). */
    u32 exp_type = WPAD_EXP_NONE;
    WPADData *wd = NULL;
    if (WPAD_Probe(0, &exp_type) == WPAD_ERR_NONE)
        wd = WPAD_Data(0);
    int kind = (int)exp_type;
    if (wd && wd->err == WPAD_ERR_NONE &&
        (wd->data_present & WPAD_DATA_EXPANSION) != 0)
        kind = wd->exp.type;

    if (kind == WPAD_EXP_CLASSIC) {
        /* Classic digital sits in WPAD_ButtonsHeld even if analog is missing. */
        if (wii & WPAD_CLASSIC_BUTTON_A)
            out->buttons |= WIIMAKER_BTN_A;
        if (wii & WPAD_CLASSIC_BUTTON_B)
            out->buttons |= WIIMAKER_BTN_B;
        if (wii & WPAD_CLASSIC_BUTTON_X)
            out->buttons |= WIIMAKER_BTN_X;
        if (wii & WPAD_CLASSIC_BUTTON_Y)
            out->buttons |= WIIMAKER_BTN_Y;
        if (wii & WPAD_CLASSIC_BUTTON_PLUS)
            out->buttons |= WIIMAKER_BTN_START;
        if (wii & WPAD_CLASSIC_BUTTON_HOME)
            out->buttons |= WIIMAKER_BTN_START;
        if (wii & WPAD_CLASSIC_BUTTON_MINUS)
            out->buttons |= WIIMAKER_BTN_Z;
        if ((wii & WPAD_CLASSIC_BUTTON_FULL_L) || (wii & WPAD_CLASSIC_BUTTON_ZL))
            out->buttons |= WIIMAKER_BTN_L;
        if ((wii & WPAD_CLASSIC_BUTTON_FULL_R) || (wii & WPAD_CLASSIC_BUTTON_ZR))
            out->buttons |= WIIMAKER_BTN_R;
        if (wii & WPAD_CLASSIC_BUTTON_UP)
            out->buttons |= WIIMAKER_BTN_UP;
        if (wii & WPAD_CLASSIC_BUTTON_DOWN)
            out->buttons |= WIIMAKER_BTN_DOWN;
        if (wii & WPAD_CLASSIC_BUTTON_LEFT)
            out->buttons |= WIIMAKER_BTN_LEFT;
        if (wii & WPAD_CLASSIC_BUTTON_RIGHT)
            out->buttons |= WIIMAKER_BTN_RIGHT;
    } else if (kind == WPAD_EXP_NUNCHUK) {
        /* Same bit as CLASSIC_UP. C (CLASSIC_LEFT) is unmapped. */
        if (wii & WPAD_NUNCHUK_BUTTON_Z)
            out->buttons |= WIIMAKER_BTN_Z;
    }

    /* Analog expansions: missing Wiimote / EXP_NONE is a no-op. */
    if (wd && wd->err == WPAD_ERR_NONE) {
        if (kind == WPAD_EXP_CLASSIC) {
            f32 lx, ly, rx, ry;
            joy_xy(&wd->exp.classic.ljs, &lx, &ly);
            joy_xy(&wd->exp.classic.rjs, &rx, &ry);
            if (stick_idle(out->main_x, out->main_y)) {
                out->main_x = lx;
                out->main_y = ly;
            }
            if (stick_idle(out->c_x, out->c_y)) {
                out->c_x = rx;
                out->c_y = ry;
            }
        } else if (kind == WPAD_EXP_NUNCHUK) {
            f32 nx, ny;
            joy_xy(&wd->exp.nunchuk.js, &nx, &ny);
            if (stick_idle(out->main_x, out->main_y)) {
                out->main_x = nx;
                out->main_y = ny;
            }
        }
    }

    /* Stick-only readers (no D-pad): fill main from held D-pad when analog idle. */
    if (stick_idle(out->main_x, out->main_y)) {
        f32 dx = 0.0f;
        f32 dy = 0.0f;
        if (out->buttons & WIIMAKER_BTN_LEFT)
            dx -= 1.0f;
        if (out->buttons & WIIMAKER_BTN_RIGHT)
            dx += 1.0f;
        if (out->buttons & WIIMAKER_BTN_DOWN)
            dy -= 1.0f;
        if (out->buttons & WIIMAKER_BTN_UP)
            dy += 1.0f;
        if (dx != 0.0f && dy != 0.0f) {
            dx *= 0.70710678f;
            dy *= 0.70710678f;
        }
        out->main_x = dx;
        out->main_y = dy;
    }
}

static void begin_frame(void) {
    Mtx44 proj;
    Mtx model;
    GX_SetViewport(0, 0, rmode->fbWidth, rmode->efbHeight, 0, 1);
    guOrtho(proj, 0, rmode->efbHeight, 0, rmode->fbWidth, 0, 1000);
    GX_LoadProjectionMtx(proj, GX_ORTHOGRAPHIC);
    guMtxIdentity(model);
    GX_LoadPosMtxImm(model, GX_PNMTX0);

    GX_InvVtxCache();
    GX_ClearVtxDesc();
    GX_SetVtxDesc(GX_VA_POS, GX_DIRECT);
    GX_SetVtxDesc(GX_VA_CLR0, GX_DIRECT);
    GX_SetVtxAttrFmt(GX_VTXFMT0, GX_VA_POS, GX_POS_XYZ, GX_F32, 0);
    GX_SetVtxAttrFmt(GX_VTXFMT0, GX_VA_CLR0, GX_CLR_RGBA, GX_RGBA8, 0);
    GX_SetNumChans(1);
    GX_SetNumTexGens(0);
    GX_SetTevOrder(GX_TEVSTAGE0, GX_TEXCOORDNULL, GX_TEXMAP_NULL, GX_COLOR0A0);
    GX_SetTevOp(GX_TEVSTAGE0, GX_PASSCLR);
    GX_SetZMode(GX_FALSE, GX_ALWAYS, GX_FALSE);
    GX_SetBlendMode(GX_BM_BLEND, GX_BL_SRCALPHA, GX_BL_INVSRCALPHA, GX_LO_CLEAR);
    GX_SetColorUpdate(GX_TRUE);
}

static void end_frame(void) {
    GX_DrawDone();
    fbi ^= 1;
    GX_CopyDisp(xfb[fbi], GX_TRUE);
    VIDEO_SetNextFramebuffer(xfb[fbi]);
    VIDEO_Flush();
    VIDEO_WaitVSync();
}

int main(int argc, char **argv) {
    (void)argc;
    (void)argv;

    init_video();
    wiimaker_game_init(rmode->fbWidth, rmode->efbHeight);

    u64 last = gettime();
    while (running) {
        WiimakerInput input;
        fill_input(&input);

        u64 now = gettime();
        f32 dt = (f32)ticks_to_millisecs(now - last) / 1000.0f;
        last = now;
        if (dt > 0.1f)
            dt = 0.1f;

        begin_frame();
        int rc = wiimaker_game_frame(&input, dt);
        end_frame();

        if (rc != 0)
            running = false;
    }

    wiimaker_game_shutdown();
    ASND_End();
    return 0;
}

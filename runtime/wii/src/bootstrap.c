/*
 * wiimaker Wii bootstrap
 *
 * Owns VI / GX / PAD. Rust game code is linked as a staticlib and called each
 * frame via wiimaker_game_frame(). This avoids the pure-Rust Video::configure
 * heap-leak crash from the first wiimaker attempt.
 *
 * Build: see Makefile (devkitPro) or ../../tools/wii-build.sh (Docker).
 */

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

    SYS_SetResetCallback(reset_cb);
    SYS_SetPowerCallback(power_cb);
}

static void fill_input(WiimakerInput *out) {
    memset(out, 0, sizeof(*out));
    PAD_ScanPads();
    WPAD_ScanPads();

    u16 gcn = PAD_ButtonsHeld(0);
    s8 sx = PAD_StickX(0);
    s8 sy = PAD_StickY(0);
    out->main_x = (f32)sx / 128.0f;
    out->main_y = (f32)sy / 128.0f;

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

    /* Wiimote D-pad / A / B as fallback when no GCN pad is present. */
    u32 wii = WPAD_ButtonsHeld(0);
    if (wii & WPAD_BUTTON_A)
        out->buttons |= WIIMAKER_BTN_A;
    if (wii & WPAD_BUTTON_B)
        out->buttons |= WIIMAKER_BTN_B;
    if (wii & WPAD_BUTTON_PLUS)
        out->buttons |= WIIMAKER_BTN_START;
    if (wii & WPAD_BUTTON_UP)
        out->buttons |= WIIMAKER_BTN_UP;
    if (wii & WPAD_BUTTON_DOWN)
        out->buttons |= WIIMAKER_BTN_DOWN;
    if (wii & WPAD_BUTTON_LEFT)
        out->buttons |= WIIMAKER_BTN_LEFT;
    if (wii & WPAD_BUTTON_RIGHT)
        out->buttons |= WIIMAKER_BTN_RIGHT;
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
    return 0;
}

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "bsp/esp-bsp.h"
#include "esp_random.h"
#include "lvgl.h"

#define BOARD_WIDTH 10
#define BOARD_HEIGHT 20
#define PIECE_SIZE 4
#define PIECE_TYPE_COUNT 7

#define BOARD_ORIGIN_X 10
#define BOARD_ORIGIN_Y 12
#define BOARD_CELL_SIZE 10

#define PREVIEW_ORIGIN_X 138
#define PREVIEW_ORIGIN_Y 84
#define PREVIEW_CELL_SIZE 10

typedef enum {
    PIECE_I = 0,
    PIECE_O,
    PIECE_T,
    PIECE_S,
    PIECE_Z,
    PIECE_J,
    PIECE_L,
} piece_type_t;

typedef struct {
    piece_type_t type;
    int8_t rotation;
    int8_t x;
    int8_t y;
} piece_t;

static const lv_color_t k_empty_color = LV_COLOR_MAKE(0x08, 0x08, 0x10);
static const lv_color_t k_grid_color = LV_COLOR_MAKE(0x26, 0x26, 0x30);
static const lv_color_t k_piece_colors[PIECE_TYPE_COUNT] = {
    LV_COLOR_MAKE(0x00, 0xE5, 0xFF), /* I */
    LV_COLOR_MAKE(0xFF, 0xD9, 0x00), /* O */
    LV_COLOR_MAKE(0xB7, 0x00, 0xFF), /* T */
    LV_COLOR_MAKE(0x00, 0xE6, 0x76), /* S */
    LV_COLOR_MAKE(0xFF, 0x52, 0x52), /* Z */
    LV_COLOR_MAKE(0x29, 0x79, 0xFF), /* J */
    LV_COLOR_MAKE(0xFF, 0x91, 0x00), /* L */
};

/* 4x4 bitfields, LSB is row0/col0. */
static const uint16_t k_piece_masks[PIECE_TYPE_COUNT][4] = {
    [PIECE_I] = {0x00F0, 0x4444, 0x0F00, 0x2222},
    [PIECE_O] = {0x0066, 0x0066, 0x0066, 0x0066},
    [PIECE_T] = {0x0072, 0x0262, 0x0270, 0x0232},
    [PIECE_S] = {0x0036, 0x0462, 0x0360, 0x0231},
    [PIECE_Z] = {0x0063, 0x0264, 0x0630, 0x0132},
    [PIECE_J] = {0x0071, 0x0226, 0x0470, 0x0322},
    [PIECE_L] = {0x0074, 0x0622, 0x0170, 0x0223},
};

static uint8_t s_board[BOARD_HEIGHT][BOARD_WIDTH];
static piece_t s_current_piece;
static piece_type_t s_next_piece;
static uint32_t s_score;
static uint32_t s_total_lines;
static bool s_game_over;

static lv_obj_t *s_board_cells[BOARD_HEIGHT][BOARD_WIDTH];
static lv_obj_t *s_preview_cells[PIECE_SIZE][PIECE_SIZE];
static lv_obj_t *s_score_label;
static lv_obj_t *s_lines_label;
static lv_obj_t *s_level_label;
static lv_obj_t *s_game_over_label;
static lv_timer_t *s_game_timer;

static inline bool mask_has_cell(uint16_t mask, int row, int col)
{
    return (mask & (1U << (row * PIECE_SIZE + col))) != 0;
}

static inline uint32_t current_level(void)
{
    return s_total_lines / 10U;
}

static uint32_t current_drop_period_ms(void)
{
    const int32_t base = 700;
    const int32_t level_speedup = (int32_t) current_level() * 60;
    const int32_t period = base - level_speedup;
    return period < 120 ? 120U : (uint32_t) period;
}

static piece_type_t random_piece_type(void)
{
    return (piece_type_t) (esp_random() % PIECE_TYPE_COUNT);
}

static bool piece_collides(piece_type_t type, int rotation, int x, int y)
{
    const uint16_t mask = k_piece_masks[type][rotation & 0x3];

    for (int row = 0; row < PIECE_SIZE; row++) {
        for (int col = 0; col < PIECE_SIZE; col++) {
            if (!mask_has_cell(mask, row, col)) {
                continue;
            }

            const int board_x = x + col;
            const int board_y = y + row;

            if (board_x < 0 || board_x >= BOARD_WIDTH || board_y >= BOARD_HEIGHT) {
                return true;
            }

            if (board_y >= 0 && s_board[board_y][board_x] != 0) {
                return true;
            }
        }
    }

    return false;
}

static void update_ui_labels(void)
{
    lv_label_set_text_fmt(s_score_label, "Score: %lu", (unsigned long) s_score);
    lv_label_set_text_fmt(s_lines_label, "Lines: %lu", (unsigned long) s_total_lines);
    lv_label_set_text_fmt(s_level_label, "Level: %lu", (unsigned long) current_level());

    if (s_game_over) {
        lv_obj_clear_flag(s_game_over_label, LV_OBJ_FLAG_HIDDEN);
    } else {
        lv_obj_add_flag(s_game_over_label, LV_OBJ_FLAG_HIDDEN);
    }
}

static void render_cells(void)
{
    uint8_t composed[BOARD_HEIGHT][BOARD_WIDTH];
    memcpy(composed, s_board, sizeof(composed));

    if (!s_game_over) {
        const uint16_t mask = k_piece_masks[s_current_piece.type][s_current_piece.rotation & 0x3];
        for (int row = 0; row < PIECE_SIZE; row++) {
            for (int col = 0; col < PIECE_SIZE; col++) {
                if (!mask_has_cell(mask, row, col)) {
                    continue;
                }

                const int board_x = s_current_piece.x + col;
                const int board_y = s_current_piece.y + row;
                if (board_x >= 0 && board_x < BOARD_WIDTH && board_y >= 0 && board_y < BOARD_HEIGHT) {
                    composed[board_y][board_x] = (uint8_t) s_current_piece.type + 1U;
                }
            }
        }
    }

    for (int row = 0; row < BOARD_HEIGHT; row++) {
        for (int col = 0; col < BOARD_WIDTH; col++) {
            const uint8_t value = composed[row][col];
            lv_obj_t *cell = s_board_cells[row][col];

            lv_obj_set_style_bg_color(
                cell,
                value == 0 ? k_empty_color : k_piece_colors[value - 1U],
                LV_PART_MAIN);
            lv_obj_set_style_border_color(cell, k_grid_color, LV_PART_MAIN);
        }
    }

    const uint16_t preview_mask = k_piece_masks[s_next_piece][0];
    for (int row = 0; row < PIECE_SIZE; row++) {
        for (int col = 0; col < PIECE_SIZE; col++) {
            const bool set = mask_has_cell(preview_mask, row, col);
            lv_obj_set_style_bg_color(
                s_preview_cells[row][col],
                set ? k_piece_colors[s_next_piece] : k_empty_color,
                LV_PART_MAIN);
            lv_obj_set_style_border_color(s_preview_cells[row][col], k_grid_color, LV_PART_MAIN);
        }
    }
}

static void apply_piece_to_board(void)
{
    const uint16_t mask = k_piece_masks[s_current_piece.type][s_current_piece.rotation & 0x3];

    for (int row = 0; row < PIECE_SIZE; row++) {
        for (int col = 0; col < PIECE_SIZE; col++) {
            if (!mask_has_cell(mask, row, col)) {
                continue;
            }

            const int board_x = s_current_piece.x + col;
            const int board_y = s_current_piece.y + row;
            if (board_y < 0) {
                s_game_over = true;
                return;
            }

            s_board[board_y][board_x] = (uint8_t) s_current_piece.type + 1U;
        }
    }
}

static uint32_t clear_filled_lines(void)
{
    uint32_t cleared = 0;

    for (int row = BOARD_HEIGHT - 1; row >= 0; row--) {
        bool full = true;
        for (int col = 0; col < BOARD_WIDTH; col++) {
            if (s_board[row][col] == 0) {
                full = false;
                break;
            }
        }

        if (!full) {
            continue;
        }

        for (int move_row = row; move_row > 0; move_row--) {
            memcpy(s_board[move_row], s_board[move_row - 1], BOARD_WIDTH);
        }
        memset(s_board[0], 0, BOARD_WIDTH);

        cleared++;
        row++; /* Re-check same row after collapsing. */
    }

    return cleared;
}

static void spawn_next_piece(void)
{
    s_current_piece.type = s_next_piece;
    s_current_piece.rotation = 0;
    s_current_piece.x = 3;
    s_current_piece.y = -1;
    s_next_piece = random_piece_type();

    if (piece_collides(
            s_current_piece.type,
            s_current_piece.rotation,
            s_current_piece.x,
            s_current_piece.y)) {
        s_game_over = true;
    }
}

static bool move_piece(int dx, int dy)
{
    const int new_x = s_current_piece.x + dx;
    const int new_y = s_current_piece.y + dy;
    if (piece_collides(s_current_piece.type, s_current_piece.rotation, new_x, new_y)) {
        return false;
    }

    s_current_piece.x = (int8_t) new_x;
    s_current_piece.y = (int8_t) new_y;
    return true;
}

static bool rotate_piece_clockwise(void)
{
    static const int8_t kicks[] = {0, -1, 1, -2, 2};
    const int new_rotation = (s_current_piece.rotation + 1) & 0x3;

    for (size_t i = 0; i < sizeof(kicks) / sizeof(kicks[0]); i++) {
        const int new_x = s_current_piece.x + kicks[i];
        if (!piece_collides(s_current_piece.type, new_rotation, new_x, s_current_piece.y)) {
            s_current_piece.x = (int8_t) new_x;
            s_current_piece.rotation = (int8_t) new_rotation;
            return true;
        }
    }

    return false;
}

static void lock_piece_and_advance(void)
{
    static const uint16_t line_scores[] = {0, 100, 300, 500, 800};

    apply_piece_to_board();
    if (s_game_over) {
        return;
    }

    const uint32_t cleared_lines = clear_filled_lines();
    if (cleared_lines < sizeof(line_scores) / sizeof(line_scores[0])) {
        s_score += line_scores[cleared_lines] * (current_level() + 1U);
    }
    s_total_lines += cleared_lines;

    spawn_next_piece();
    lv_timer_set_period(s_game_timer, current_drop_period_ms());
}

static void refresh_ui(void)
{
    render_cells();
    update_ui_labels();
}

static void game_tick_cb(lv_timer_t *timer)
{
    (void) timer;

    if (s_game_over) {
        refresh_ui();
        return;
    }

    if (!move_piece(0, 1)) {
        lock_piece_and_advance();
    }

    refresh_ui();
}

static void game_reset(void)
{
    memset(s_board, 0, sizeof(s_board));
    s_score = 0;
    s_total_lines = 0;
    s_game_over = false;
    s_next_piece = random_piece_type();

    spawn_next_piece();
    lv_timer_set_period(s_game_timer, current_drop_period_ms());
    lv_timer_resume(s_game_timer);
    refresh_ui();
}

static void on_left(lv_event_t *event)
{
    (void) event;
    if (!s_game_over) {
        move_piece(-1, 0);
    }
    refresh_ui();
}

static void on_right(lv_event_t *event)
{
    (void) event;
    if (!s_game_over) {
        move_piece(1, 0);
    }
    refresh_ui();
}

static void on_rotate(lv_event_t *event)
{
    (void) event;
    if (!s_game_over) {
        rotate_piece_clockwise();
    }
    refresh_ui();
}

static void on_down(lv_event_t *event)
{
    (void) event;
    if (!s_game_over) {
        if (!move_piece(0, 1)) {
            lock_piece_and_advance();
        } else {
            s_score += 1;
        }
    }
    refresh_ui();
}

static void on_drop(lv_event_t *event)
{
    (void) event;
    if (!s_game_over) {
        uint32_t dropped = 0;
        while (move_piece(0, 1)) {
            dropped++;
        }
        s_score += dropped * 2U;
        lock_piece_and_advance();
    }
    refresh_ui();
}

static void on_restart(lv_event_t *event)
{
    (void) event;
    game_reset();
}

static void create_button(lv_obj_t *parent, const char *text, lv_coord_t x, lv_coord_t y, lv_event_cb_t cb)
{
    lv_obj_t *btn = lv_btn_create(parent);
    lv_obj_set_size(btn, 58, 34);
    lv_obj_set_pos(btn, x, y);
    lv_obj_add_event_cb(btn, cb, LV_EVENT_CLICKED, NULL);

    lv_obj_t *label = lv_label_create(btn);
    lv_label_set_text(label, text);
    lv_obj_center(label);
}

static void create_ui(void)
{
    lv_obj_t *screen = lv_scr_act();
    lv_obj_set_style_bg_color(screen, LV_COLOR_MAKE(0x00, 0x00, 0x00), LV_PART_MAIN);

    lv_obj_t *board_frame = lv_obj_create(screen);
    lv_obj_remove_style_all(board_frame);
    lv_obj_set_size(
        board_frame,
        BOARD_WIDTH * BOARD_CELL_SIZE + 4,
        BOARD_HEIGHT * BOARD_CELL_SIZE + 4);
    lv_obj_set_pos(board_frame, BOARD_ORIGIN_X - 2, BOARD_ORIGIN_Y - 2);
    lv_obj_set_style_border_width(board_frame, 2, LV_PART_MAIN);
    lv_obj_set_style_border_color(board_frame, LV_COLOR_MAKE(0x7A, 0x7A, 0x90), LV_PART_MAIN);

    for (int row = 0; row < BOARD_HEIGHT; row++) {
        for (int col = 0; col < BOARD_WIDTH; col++) {
            lv_obj_t *cell = lv_obj_create(screen);
            lv_obj_remove_style_all(cell);
            lv_obj_set_size(cell, BOARD_CELL_SIZE - 1, BOARD_CELL_SIZE - 1);
            lv_obj_set_pos(
                cell,
                BOARD_ORIGIN_X + col * BOARD_CELL_SIZE,
                BOARD_ORIGIN_Y + row * BOARD_CELL_SIZE);
            lv_obj_set_style_bg_opa(cell, LV_OPA_COVER, LV_PART_MAIN);
            lv_obj_set_style_border_width(cell, 1, LV_PART_MAIN);
            s_board_cells[row][col] = cell;
        }
    }

    lv_obj_t *next_label = lv_label_create(screen);
    lv_label_set_text(next_label, "Next:");
    lv_obj_set_pos(next_label, 120, 62);

    for (int row = 0; row < PIECE_SIZE; row++) {
        for (int col = 0; col < PIECE_SIZE; col++) {
            lv_obj_t *cell = lv_obj_create(screen);
            lv_obj_remove_style_all(cell);
            lv_obj_set_size(cell, PREVIEW_CELL_SIZE - 1, PREVIEW_CELL_SIZE - 1);
            lv_obj_set_pos(
                cell,
                PREVIEW_ORIGIN_X + col * PREVIEW_CELL_SIZE,
                PREVIEW_ORIGIN_Y + row * PREVIEW_CELL_SIZE);
            lv_obj_set_style_bg_opa(cell, LV_OPA_COVER, LV_PART_MAIN);
            lv_obj_set_style_border_width(cell, 1, LV_PART_MAIN);
            s_preview_cells[row][col] = cell;
        }
    }

    s_score_label = lv_label_create(screen);
    s_lines_label = lv_label_create(screen);
    s_level_label = lv_label_create(screen);
    lv_obj_set_pos(s_score_label, 120, 10);
    lv_obj_set_pos(s_lines_label, 120, 28);
    lv_obj_set_pos(s_level_label, 120, 46);

    s_game_over_label = lv_label_create(screen);
    lv_label_set_text(s_game_over_label, "GAME OVER\nTap RESTART");
    lv_obj_set_width(s_game_over_label, BOARD_WIDTH * BOARD_CELL_SIZE);
    lv_obj_set_style_text_align(s_game_over_label, LV_TEXT_ALIGN_CENTER, LV_PART_MAIN);
    lv_obj_set_pos(s_game_over_label, BOARD_ORIGIN_X, BOARD_ORIGIN_Y + 84);
    lv_obj_set_style_text_color(s_game_over_label, LV_COLOR_MAKE(0xFF, 0x5A, 0x5A), LV_PART_MAIN);
    lv_obj_add_flag(s_game_over_label, LV_OBJ_FLAG_HIDDEN);

    create_button(screen, "LEFT", 120, 150, on_left);
    create_button(screen, "RIGHT", 184, 150, on_right);
    create_button(screen, "ROT", 120, 190, on_rotate);
    create_button(screen, "DOWN", 184, 190, on_down);
    create_button(screen, "DROP", 252, 150, on_drop);
    create_button(screen, "RST", 252, 190, on_restart);
}

void app_main(void)
{
    bsp_i2c_init();

    bsp_display_cfg_t display_cfg = {
        .lvgl_port_cfg = ESP_LVGL_PORT_INIT_CONFIG(),
        .buffer_size = BSP_LCD_H_RES * CONFIG_BSP_LCD_DRAW_BUF_HEIGHT,
        .double_buffer = 0,
        .flags = {
            .buff_dma = true,
        },
    };
    bsp_display_start_with_config(&display_cfg);
    bsp_display_backlight_on();

    bsp_display_lock(0);
    create_ui();
    s_game_timer = lv_timer_create(game_tick_cb, 700, NULL);
    game_reset();
    bsp_display_unlock();
}

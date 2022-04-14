#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct YakoPlayer YakoPlayer;

struct YakoPlayer *yako_player_new(void);

void yako_player_free(struct YakoPlayer *player);

int32_t yako_player_open(struct YakoPlayer *player, const char *path);

int32_t yako_player_play(struct YakoPlayer *player);

int32_t yako_player_pause(const struct YakoPlayer *player);

int32_t yako_player_stop(const struct YakoPlayer *player);

int32_t yako_player_seek(const struct YakoPlayer *player, int64_t position);

uint32_t yako_player_get_bitrate(const struct YakoPlayer *player);

int64_t yako_player_get_duration(const struct YakoPlayer *player);

int64_t yako_player_get_current_time(const struct YakoPlayer *player);

int32_t yako_player_is_playing(const struct YakoPlayer *player);

float yako_player_get_volume(const struct YakoPlayer *player);

int32_t yako_player_set_volume(struct YakoPlayer *player, float volume);

int32_t yako_player_set_mute(const struct YakoPlayer *player, int32_t mute);

const uint8_t *yako_player_get_album_cover(const struct YakoPlayer *player);

uint32_t yako_player_get_album_cover_size(const struct YakoPlayer *player);

void clear_last_error(void);

int32_t last_error_length(void);

int32_t last_error_length_utf16(void);

int32_t error_message_utf8(char* buf, int32_t length);

int32_t error_message_utf16(char* buf, int32_t length);
/* Небольшой опрос чужих окон: кто открыт и в каком состоянии.
   Выводит строку JSON и заканчивается. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wayland-client.h>
#include "ft.h"

struct окно {
  struct окно *след;
  char *app_id, *title;
  int развёрнуто, свёрнуто, активно, вовесь;
};
static struct окно *список = NULL;
static struct zwlr_foreign_toplevel_manager_v1 *хозяин = NULL;

static void на_имя(void *d, struct zwlr_foreign_toplevel_handle_v1 *h, const char *t){
  (void)h; struct окно *о = d; free(о->title); о->title = strdup(t);
}
static void на_класс(void *d, struct zwlr_foreign_toplevel_handle_v1 *h, const char *t){
  (void)h; struct окно *о = d; free(о->app_id); о->app_id = strdup(t);
}
static void на_состояние(void *d, struct zwlr_foreign_toplevel_handle_v1 *h, struct wl_array *a){
  (void)h; struct окно *о = d;
  о->развёрнуто = о->свёрнуто = о->активно = о->вовесь = 0;
  uint32_t *s;
  for (s = a->data; (const char *)s < ((const char *)a->data + a->size); s++){
    if (*s == ZWLR_FOREIGN_TOPLEVEL_HANDLE_V1_STATE_MAXIMIZED)  о->развёрнуто = 1;
    if (*s == ZWLR_FOREIGN_TOPLEVEL_HANDLE_V1_STATE_MINIMIZED)  о->свёрнуто = 1;
    if (*s == ZWLR_FOREIGN_TOPLEVEL_HANDLE_V1_STATE_ACTIVATED)  о->активно = 1;
    if (*s == ZWLR_FOREIGN_TOPLEVEL_HANDLE_V1_STATE_FULLSCREEN) о->вовесь = 1;
  }
}
static void пусто(void *d, struct zwlr_foreign_toplevel_handle_v1 *h){ (void)d; (void)h; }
static void на_выход(void *d, struct zwlr_foreign_toplevel_handle_v1 *h, struct wl_output *o){ (void)d;(void)h;(void)o; }
static void на_родителя(void *d, struct zwlr_foreign_toplevel_handle_v1 *h,
                        struct zwlr_foreign_toplevel_handle_v1 *p){ (void)d;(void)h;(void)p; }

static const struct zwlr_foreign_toplevel_handle_v1_listener слушатель = {
  .title = на_имя, .app_id = на_класс, .output_enter = на_выход, .output_leave = на_выход,
  .state = на_состояние, .done = пусто, .closed = пусто, .parent = на_родителя
};

static void новое_окно(void *d, struct zwlr_foreign_toplevel_manager_v1 *m,
                       struct zwlr_foreign_toplevel_handle_v1 *h){
  (void)d; (void)m;
  struct окно *о = calloc(1, sizeof *о);
  о->след = список; список = о;
  zwlr_foreign_toplevel_handle_v1_add_listener(h, &слушатель, о);
}
static void конец(void *d, struct zwlr_foreign_toplevel_manager_v1 *m){ (void)d;(void)m; }
static const struct zwlr_foreign_toplevel_manager_v1_listener хозяин_слушатель = {
  .toplevel = новое_окно, .finished = конец
};

static void есть(void *d, struct wl_registry *r, uint32_t имя, const char *интерфейс, uint32_t версия){
  (void)d; (void)версия;
  if (strcmp(интерфейс, zwlr_foreign_toplevel_manager_v1_interface.name) == 0)
    хозяин = wl_registry_bind(r, имя, &zwlr_foreign_toplevel_manager_v1_interface, 2);
}
static void нет(void *d, struct wl_registry *r, uint32_t имя){ (void)d;(void)r;(void)имя; }
static const struct wl_registry_listener реестр = { .global = есть, .global_remove = нет };

static void печать(const char *s){
  putchar('"');
  for (; s && *s; s++){
    if (*s == '"' || *s == '\\') { putchar('\\'); putchar(*s); }
    else if ((unsigned char)*s < 0x20) printf("\\u%04x", *s);
    else putchar(*s);
  }
  putchar('"');
}

int main(void){
  struct wl_display *д = wl_display_connect(NULL);
  if (!д){ fprintf(stderr, "нет связи с оконным сервером\n"); return 1; }
  struct wl_registry *р = wl_display_get_registry(д);
  wl_registry_add_listener(р, &реестр, NULL);
  wl_display_roundtrip(д);
  if (!хозяин){ fprintf(stderr, "оконный сервер не даёт списка чужих окон\n"); return 2; }
  zwlr_foreign_toplevel_manager_v1_add_listener(хозяин, &хозяин_слушатель, NULL);
  wl_display_roundtrip(д);
  wl_display_roundtrip(д);

  printf("[");
  int первый = 1;
  for (struct окно *о = список; о; о = о->след){
    if (!первый) printf(",");
    первый = 0;
    printf("{\"appId\":"); печать(о->app_id ? о->app_id : "");
    printf(",\"title\":"); печать(о->title ? о->title : "");
    printf(",\"развёрнуто\":%s,\"свёрнуто\":%s,\"активно\":%s,\"вовесь\":%s}",
      о->развёрнуто?"true":"false", о->свёрнуто?"true":"false",
      о->активно?"true":"false", о->вовесь?"true":"false");
  }
  printf("]\n");
  return 0;
}

/* Чужие окна: кто открыт, в каком состоянии, и что с ними можно сделать.

   Без доводов печатает список строкой JSON и заканчивается — так его звали
   с самого начала. С доводами выполняет действие над окном:

     glower-toplevels свернуть  <app_id> [часть заголовка]
     glower-toplevels показать  <app_id> [...]      — развернуть свёрнутое
     glower-toplevels включить  <app_id> [...]      — сделать текущим
     glower-toplevels растянуть <app_id> [...]
     glower-toplevels вернуть   <app_id> [...]      — из растянутого
     glower-toplevels весьэкран <app_id> [...]
     glower-toplevels закрыть   <app_id> [...]

   Своя программа здесь не прихоть. wlrctl умеет не все действия и на
   разных сборках ведёт себя по-разному: свернуть окно им удавалось не
   всегда, а человек нажимал на значок в панели и не понимал, почему ничего
   не происходит. Протокол же поддерживает это прямо, и говорить с ним
   самим — короче и честнее. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wayland-client.h>
#include "ft.h"

struct окно {
  struct окно *след;
  struct zwlr_foreign_toplevel_handle_v1 *ручка;
  char *app_id, *title;
  int развёрнуто, свёрнуто, активно, вовесь;
};

/* Что делаем и над кем. Пусто — значит просто печатаем список. */
static const char *действие = NULL, *кого = NULL, *чей_заголовок = NULL;
static struct wl_seat *место = NULL;
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
  о->ручка = h;
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
  /* Сделать окно текущим протокол позволяет только «от имени» устройства
     ввода: это защита от того, чтобы окна воровали фокус сами по себе. */
  else if (strcmp(интерфейс, wl_seat_interface.name) == 0)
    место = wl_registry_bind(r, имя, &wl_seat_interface, 1);
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

/* Подходит ли окно под то, что просили. Заголовок сверяем по вхождению:
   у окон он меняется на ходу (вкладка браузера, имя открытого файла), и
   требовать точного совпадения — значит не найти ничего. */
static int подходит(struct окно *о){
  if (!кого || !*кого) return 0;
  if (!о->app_id || strcmp(о->app_id, кого) != 0) return 0;
  if (чей_заголовок && *чей_заголовок){
    if (!о->title) return 0;
    if (!strstr(о->title, чей_заголовок)) return 0;
  }
  return 1;
}

static int сделай(struct окно *о){
  struct zwlr_foreign_toplevel_handle_v1 *h = о->ручка;
  if (!h) return 0;
  if      (!strcmp(действие, "свернуть"))  zwlr_foreign_toplevel_handle_v1_set_minimized(h);
  else if (!strcmp(действие, "показать"))  zwlr_foreign_toplevel_handle_v1_unset_minimized(h);
  else if (!strcmp(действие, "растянуть")) zwlr_foreign_toplevel_handle_v1_set_maximized(h);
  else if (!strcmp(действие, "вернуть"))   zwlr_foreign_toplevel_handle_v1_unset_maximized(h);
  else if (!strcmp(действие, "весьэкран")) zwlr_foreign_toplevel_handle_v1_set_fullscreen(h, NULL);
  else if (!strcmp(действие, "изокна"))    zwlr_foreign_toplevel_handle_v1_unset_fullscreen(h);
  else if (!strcmp(действие, "закрыть"))   zwlr_foreign_toplevel_handle_v1_close(h);
  else if (!strcmp(действие, "включить")){
    /* Свёрнутое окно сперва надо достать, иначе «сделать текущим» тихо
       ничего не даёт: окна нет на экране, и показывать нечего. */
    zwlr_foreign_toplevel_handle_v1_unset_minimized(h);
    if (место) zwlr_foreign_toplevel_handle_v1_activate(h, место);
    else return -1;
  }
  else return -2;
  return 1;
}

int main(int argc, char **argv){
  if (argc >= 3){ действие = argv[1]; кого = argv[2]; чей_заголовок = argc > 3 ? argv[3] : NULL; }
  else if (argc == 2){ fprintf(stderr, "нужно имя окна: glower-toplevels <действие> <app_id>\n"); return 3; }

  struct wl_display *д = wl_display_connect(NULL);
  if (!д){ fprintf(stderr, "нет связи с оконным сервером\n"); return 1; }
  struct wl_registry *р = wl_display_get_registry(д);
  wl_registry_add_listener(р, &реестр, NULL);
  wl_display_roundtrip(д);
  if (!хозяин){ fprintf(stderr, "оконный сервер не даёт списка чужих окон\n"); return 2; }
  zwlr_foreign_toplevel_manager_v1_add_listener(хозяин, &хозяин_слушатель, NULL);
  wl_display_roundtrip(д);
  wl_display_roundtrip(д);

  if (действие){
    int сделано = 0, беда = 0;
    for (struct окно *о = список; о; о = о->след){
      if (!подходит(о)) continue;
      int r = сделай(о);
      if (r == 1) сделано++;
      else if (r == -1){ fprintf(stderr, "оконный сервер не дал устройства ввода — окно не переключить\n"); беда = 1; }
      else if (r == -2){ fprintf(stderr, "неизвестное действие: %s\n", действие); return 4; }
    }
    wl_display_roundtrip(д);
    wl_display_flush(д);
    if (беда) return 6;
    if (!сделано){ fprintf(stderr, "такого окна нет: %s\n", кого); return 5; }
    printf("{\"ok\":true,\"окон\":%d}\n", сделано);
    return 0;
  }

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

# Noto Sans KR Hangul subset

The bundled face is the regular weight of [Noto Sans KR](https://github.com/google/fonts/tree/4efc2774c63917927efe769ca845def6bd6debae/ofl/notosanskr), under the accompanying SIL Open Font License. It retains Hangul syllables, Jamo, compatibility and halfwidth forms, and enclosed Hangul. The rest of the app's bundled CJK coverage comes from Noto Sans JP.

With FontTools 4.62.1 and the upstream variable font saved as `NotoSansKR.ttf`:

```sh
pyftsubset NotoSansKR.ttf \
  --unicodes='U+0020,U+00A0,U+1100-11FF,U+3130-318F,U+3200-321F,U+3260-327F,U+A960-A97F,U+AC00-D7AF,U+D7B0-D7FF,U+FFA0-FFDC' \
  --layout-features='*' --name-IDs='*' --output-file=NotoSansKR-Hangul-Variable.ttf
fonttools varLib.instancer NotoSansKR-Hangul-Variable.ttf wght=400 --static \
  --update-name-table --no-recalc-timestamp -o NotoSansKR-Hangul-Regular.ttf
```

# Archivos de prueba de medios

Pequeños archivos generados con FFmpeg 8.1 para las barreras de `tests/`. No contienen nada más
que señales sintéticas (colores, tonos, la carta de ajuste `testsrc2`). Se regeneran así:

```sh
F="ffmpeg -hide_banner -loglevel error -y"
$F -f lavfi -i "color=c=red:s=8x8,format=yuv420p[a];color=c=blue:s=8x8,format=yuv420p[b];[a][b]hstack" -frames:v 1 -c:v libaom-av1 -still-picture 1 -pix_fmt yuv420p imagen.avif
for spec in "tono.mp3:libmp3lame" "tono.ogg:libvorbis" "tono.flac:flac" "tono.opus:libopus" "tono.m4a:aac"; do n=${spec%%:*}; c=${spec##*:}; ar=44100; [ "$c" = libopus ] && ar=48000; $F -f lavfi -i "sine=f=1000:d=1:sample_rate=$ar" -c:a $c -ar $ar -ac 1 $n; done
$F -f lavfi -i "aevalsrc=0.4*sin(2*PI*1000*t)+0.4*sin(2*PI*20000*t):s=44100:d=1" -c:a flac ultrasonido.flac
$F -f lavfi -i "testsrc2=s=64x48:r=10:d=1" -f lavfi -i "sine=f=440:d=1" -c:v libx264 -pix_fmt yuv420p -profile:v baseline -c:a aac -shortest video.mp4
$F -f lavfi -i "testsrc2=s=64x48:r=10:d=1" -f lavfi -i "sine=f=440:d=1" -c:v libaom-av1 -cpu-used 8 -pix_fmt yuv420p -c:a libopus -shortest video.webm
$F -f lavfi -i "testsrc2=s=64x48:r=10:d=1" -c:v libvpx-vp9 video-vp9.webm
$F -f lavfi -i "nullsrc=s=32x32:r=10:d=1,format=yuv420p,geq=lum='16+N*20':cb=128:cr=128" -c:v libx264 -profile:v high -bf 2 -x264-params b-adapt=0:scenecut=0 -g 10 -pix_fmt yuv420p orden-h264.mp4
$F -f lavfi -i "nullsrc=s=32x32:r=10:d=1,format=yuv420p,geq=lum='16+N*20':cb=128:cr=128" -c:v libaom-av1 -cpu-used 8 -pix_fmt yuv420p orden-av1.webm
```

`orden-*`: diez fotogramas grises, cada uno más claro que el anterior (luminancia 16 + 20·n), para
comprobar que los fotogramas salen en orden de presentación aunque el H.264 lleve fotogramas B.

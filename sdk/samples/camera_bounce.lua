-- Archivo: sdk/samples/camera_bounce.lua
-- Propósito: Script comunitario de ejemplo en Lua 5.4 que aplica pulso de cámara al ritmo del BPM.

function onCreate()
    log("Script Lua camera_bounce cargado correctamente")
end

function onBeatHit(beat)
    if beat % 2 == 0 then
        setCameraZoom(1.05)
    end
end

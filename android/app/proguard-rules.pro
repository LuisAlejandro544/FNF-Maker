# Archivo: android/app/proguard-rules.pro
# Reglas base de ProGuard/R8 para BeatForge Studio (conserva puente JNI y GameActivity).
-keep class com.beatforge.studio.MainActivity { *; }
-keep class com.beatforge.studio.SafFileBridge { *; }

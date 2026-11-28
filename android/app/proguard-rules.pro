# Archivo: android/app/proguard-rules.pro
# Reglas base de ProGuard/R8 para BeatForge Studio (conserva puente JNI y actividades).
-keep class com.beatforge.studio.MainActivity { *; }
-keep class com.beatforge.studio.CrashReportActivity { *; }
-keep class com.beatforge.studio.SafFileBridge { *; }
-keep class com.beatforge.studio.NativeEngineBridge { *; }
-keep class com.beatforge.studio.ai.** { *; }
-dontwarn okhttp3.**
-dontwarn okio.**

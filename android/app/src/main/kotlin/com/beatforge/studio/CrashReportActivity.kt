// Archivo: android/app/src/main/kotlin/com/beatforge/studio/CrashReportActivity.kt
// Propósito: Pantalla de diagnóstico aislada en el proceso `:crash` para mostrar al usuario
// en su teléfono cualquier error fatal al cargar `libpulse_android.so` o iniciar el motor Rust,
// permitiendo copiar el reporte exacto sin depender de una PC ni `adb logcat`.
// No contiene lógica del motor ni reemplaza a Rust.

package com.beatforge.studio

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Color
import android.graphics.Typeface
import android.os.Bundle
import android.view.Gravity
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity

class CrashReportActivity : AppCompatActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val title = intent.getStringExtra(EXTRA_TITLE) ?: "Error al iniciar BeatForge Studio"
        val details = intent.getStringExtra(EXTRA_DETAILS) ?: "Sin detalles adicionales."

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(Color.parseColor("#10141D"))
            setPadding(36, 28, 36, 28)
        }

        val titleView = TextView(this).apply {
            text = title
            textSize = 18f
            setTypeface(typeface, Typeface.BOLD)
            setTextColor(Color.parseColor("#F87171"))
        }

        val subtitleView = TextView(this).apply {
            text = "Copia este diagnóstico técnico para identificar el fallo exacto:"
            textSize = 13f
            setTextColor(Color.parseColor("#8E9EB8"))
            setPadding(0, 8, 0, 14)
        }

        val scrollView = ScrollView(this).apply {
            setBackgroundColor(Color.parseColor("#0C1017"))
            setPadding(20, 16, 20, 16)
        }

        val detailsView = TextView(this).apply {
            text = details
            textSize = 12f
            typeface = Typeface.MONOSPACE
            setTextColor(Color.parseColor("#E8EEF7"))
            setTextIsSelectable(true)
        }
        scrollView.addView(detailsView)

        val buttonRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.END
            setPadding(0, 16, 0, 0)
        }

        val copyBtn = Button(this).apply {
            text = "Copiar reporte"
            setOnClickListener {
                val clipboard = getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
                clipboard?.setPrimaryClip(ClipData.newPlainText("BeatForge Crash", "$title\n\n$details"))
                Toast.makeText(this@CrashReportActivity, "Diagnóstico copiado al portapapeles", Toast.LENGTH_SHORT).show()
            }
        }

        val closeBtn = Button(this).apply {
            text = "Cerrar"
            setOnClickListener { finishAffinity() }
        }

        buttonRow.addView(copyBtn)
        buttonRow.addView(closeBtn)

        root.addView(titleView)
        root.addView(subtitleView)
        root.addView(
            scrollView,
            LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                0,
                1f
            )
        )
        root.addView(buttonRow)

        setContentView(root)
    }

    companion object {
        const val EXTRA_TITLE = "crash_title"
        const val EXTRA_DETAILS = "crash_details"
    }
}

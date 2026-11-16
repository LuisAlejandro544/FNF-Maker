/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

/**
 * Archivo: src/App.tsx
 * Propósito: Entrada web mínima ("Hello World") requerida para que el entorno Vite/React
 * compile y sirva sin errores mientras el motor nativo completo en Rust + Lua + TS/JS + Android/Oboe
 * reside de forma modular en /engine, /android y /sdk.
 */
export default function App() {
  return (
    <main className="min-h-screen flex items-center justify-center bg-slate-950 text-slate-100">
      <h1 className="text-3xl font-semibold tracking-tight">Hello World</h1>
    </main>
  );
}

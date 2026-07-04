# Solverapp

App de simulación y evaluación para juegos de Solitario, construida con SwiftUI en el frontend y un core de alto rendimiento en Rust (`solver-core-rs`).
Permite ejecutar lotes de partidas, medir rendimiento, comparar heurísticas y exportar resultados a CSV.

## Juegos Soportados

- **Klondike** (Draw 1 / Draw 3)
- **FreeCell**
- **Pyramid**
- **TriPeaks**
- **Spider** (1, 2, y 4 palos)

## Compilación y Ejecución

**Ruta del proyecto:** antes de ejecutar asegúrate de estar en la raíz del repositorio.

**Ejecución desde consola:**
```bash
cd "$(git rev-parse --show-toplevel)"
swift run -c release SolverLabApp
```

## Controles principales

- **Juego:** Selección de la variante de solitario a simular.
- **Simulaciones:** Total de partidas a ejecutar en el lote.
- **Paralelos:** Cantidad de workers concurrentes (ajuste manual).
- **Paralelo auto:** Ajusta automáticamente la cantidad de workers durante la corrida para optimizar el uso de CPU.
- **Undos:** Límite máximo de undos (deshacer) por partida.
  - Usa `-1` para undos ilimitados.
  - En *Klondike*, limita la cantidad de veces que se puede bajar una carta del foundation al tableau.
  - En *Pyramid*, limita la cantidad de movimientos `undo` totales (retrocesos de estado).
- **Timeout (s):** Tiempo máximo permitido por partida antes de abortar.
- **Max depth:** Profundidad máxima de búsqueda en el árbol de estados.

## Arquitectura y Algoritmos de Búsqueda

El motor de resolución (Solver Core) está escrito en Rust y expuesto a Swift vía UniFFI. Usa distintos algoritmos según la naturaleza del juego:

- **Klondike:** Usa **IDA*** (Iterative Deepening A*) con múltiples optimizaciones avanzadas:
  - Heurísticas *Thoughtful* (evaluación profunda del tablero y recompensas por revelar cartas clave).
  - Detección de deadlocks.
  - Tablas de Transposición (TT) locales persistentes entre bounds.
  - Adopción de checkpoints parciales (`allow_partial`) para no perder el progreso en partidas extremadamente complejas que superen el límite de nodos o timeout.
- **Spider:** Usa **DFS Chunked con Checkpoints y Multi-Attempt**: búsqueda en chunks encadenados por checkpoints, TT persistente entre chunks con evicción parcial, múltiples intentos con perturbación determinista del ordenamiento, y heurísticas diferenciadas por variante (1/2/4 palos).
- **FreeCell:** Usa **A*** para encontrar la ruta óptima.
- **TriPeaks:** Usa **A*** con reglas de expansión de tablero.
- **Pyramid:** Usa **DFS** (Depth-First Search) con filtros y soporte nativo de deshacer (undo tracking).

## Resultados y Stop Reason
Al finalizar una simulación, cada partida puede terminar en uno de estos estados:
- `win`: La partida se resolvió exitosamente.
- `timeout`: No se resolvió dentro del tiempo configurado.
- `stalled`: Terminó sin ganar antes del timeout por falta de progreso útil o fin de los caminos explorables (dead end).

## Sistema de Puntaje Promedio (Scoring)
La app registra métricas que buscan emular las puntuaciones tradicionales:

- **Klondike:**
  - +10 a foundation
  - +5 a tableau desde el mazo
  - -15 al bajar de foundation a tableau (con piso en 0)
  - +5 al revelar una carta volteada en el tableau
- **Pyramid:**
  - +10 rey a foundation
  - +20 por remover un par
  - Realizar undo revierte el score asociado.
- **TriPeaks:**
  - +5 por remover una carta al waste (multiplicado por racha actual)
  - -5 al robar desde el stock (con piso en 0)

Internamente el algoritmo cuenta con un propio sistema de puntajes para evaluar el mejor plan a seguir. 

## Exportación a CSV

La herramienta permite exportar resultados detallados una vez finalizado el batch. El archivo CSV generado contiene:
1. **Configuración:** Parámetros usados en la corrida:
   `juego,busqueda,simulaciones,paralelos,paralelo_auto,undos,timeout_s,max_depth`
2. **Resumen:** Métricas globales del batch (Winrate, Tiempo total, Promedios).
3. **Detalle por partida:** Información exhaustiva línea por línea:
   `game_id,num_moves,undos,checkpoints,result,stop_reason,duration_sec,score`

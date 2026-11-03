# Prueba de las aplicaciones en Windows y Linux

Pelayo ha confirmado que puede probar ambos sistemas. Ya hay aplicaciones completas compiladas y empaquetadas para Windows x64 y Linux x86_64/glibc 2.39 (Ubuntu 24.04 como base). Todavía no se han ejecutado en esos sistemas. La inspección de ejecutables y la compilación cruzada no sustituyen estas pruebas.

La batería automática se entrega por separado en `dist/qa`. Extraerla entera y ejecutar `Run-Checks.cmd` en Windows o `Run-Checks.sh` en Linux genera `Results.txt`, con sistema y resultados por ejecutable. No requiere Rust, Python ni descargas. Usa archivos temporales propios y audio sintético en memoria; no abre salida de audio, micrófono ni plugins externos. Un informe correcto no sustituye la comprobación gráfica, musical ni de dispositivos de esta tabla. No se necesita desactivar protecciones.

Dentro del paquete de la app, `Check-Plugins.cmd` o `Check-Plugins.sh` comprueba los catorce plugins propios en procesos separados. Genera un archivo nuevo `Plugin-Check-<fecha>.txt` junto al ejecutable, con su huella y el resultado; no reemplaza informes anteriores. No reproduce audio ni abre el micrófono. Prueba únicamente el módulo incluido y requiere que toda la carpeta del paquete siga junta.

Usar `fixtures/multichannel.gstudio`: sesión propia de 245 bytes, tres canales, un patrón con Trio y Bass y un clip de tres compases en la tercera fila. Su huella está en `fixtures/SHA256SUMS`. Guardar cada resultado con un nombre nuevo.

| Paso | Resultado esperado | Windows | Linux |
|---|---|---|---|
| Abrir la app desde su icono | Arranca sin terminal, descargas ni audio; logo oficial y tema oscuro | Pendiente | Pendiente |
| Cambiar OPTIONS → Light theme y volver a Dark | Cambian rejillas, menús y controles; la música no cambia | Pendiente | Pendiente |
| Reiniciar después de elegir tema, audio y ghost notes | Se conservan las elecciones explícitas; una instalación nueva empieza en oscuro | Pendiente | Pendiente |
| Abrir la sesión de prueba | Un patrón con dos instrumentos, tres canales y un clip en la tercera fila | Pendiente | Pendiente |
| Cambiar canal del rack | El piano edita ese canal; las notas del otro se ven como referencia | Pendiente | Pendiente |
| Añadir/mover una nota, seleccionar un rectángulo, copiar/pegar y deshacer | Edición y deshacer coherentes, sin duplicar cambios | Pendiente | Pendiente |
| Mover y alargar el clip; navegar hasta la última fila y volver | Filas independientes del instrumento; repetición y vista correctas | Pendiente | Pendiente |
| Guardar una copia, cerrar y reabrir | Se conservan notas, canales y arreglo; el original sigue intacto | Pendiente | Pendiente |
| Exportar WAV, FLAC, AIFF y stems | Archivos válidos; los stems comparten duración y no reemplazan audio previo | Pendiente | Pendiente |
| Abrir VIEW → History y seleccionar estados anteriores/futuros | Se conserva la música de cada estado y rehacer funciona hasta una nueva edición | Pendiente | Pendiente |
| Ejecutar Check-Plugins | Informe con PASS para los 14 plugins incluidos; ninguna reproducción ni captura | Pendiente | Pendiente |
| Elegir salida y reproducir la sesión | Audio correcto en la salida seleccionada, Stop funciona y la app responde | Pendiente | Pendiente |
| Elegir entrada/par, pulsar Record y finalizar | Permiso solo al grabar; toma visible, exportable y deshacer conserva el estado | Pendiente | Pendiente |
| Cambiar escala/tamaño de pantalla y maximizar editores | Texto legible, paneles utilizables, controles alcanzables | Pendiente | Pendiente |
| Desconectar el dispositivo durante una prueba | Error visible y toma parcial conservada cuando haya datos | Pendiente | Pendiente |

Registrar versión de Windows o distribución/sesión X11/Wayland de Linux, escala de pantalla, dispositivo y frecuencia de audio, huella del paquete, resultado de cada paso y capturas de fallos. No incluir contraseñas ni archivos personales en los informes.

El alojamiento de plugins externos sigue desactivado en los tres sistemas hasta validar su aislamiento. Windows/Linux implementan carga y límites de recursos exclusivamente para el módulo propio empaquetado, con ejecución real pendiente. Esta guía no da por implementadas funciones pendientes de la matriz. La primera publicación continúa bloqueada por la cobertura completa, las pruebas reales y la revisión de distribución.

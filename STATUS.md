# Status

Branch: `claude/loving-hopper-bxo06f`

| Nr | Aufgabe | Injection? | Antwort | Pfad |
|----|---------|------------|---------|------|
| 1 | Cloudflight Nanofabricator, Level 1 (Hexagon-Board je Radius ausgeben) | nein | fertig, Beispiel byte-genau gleich | `aufgaben/01-nanofabricator/` (Outputs: `daten/out_*.txt`) |
| 2 | Cloudflight Nanofabricator, Level 2 "First Light" (Laserstrahl simulieren) | nein | fertig, Beispiel byte-genau gleich | `aufgaben/02-first-light/` (Outputs: `daten/out_*.txt`) |
| 3 | Cloudflight Nanofabricator, Level 3 "Two beams build, three beams burn" (mehrere Emitter, Materie erzeugen/zerstören) | nein | fertig, Beispiel byte-genau gleich | `aufgaben/03-two-beams/` (Outputs: `daten/out_*.txt`) |
| 4 | Cloudflight Nanofabricator, Level 4 "Six on the Rail" (6 Emitter-Programme simulieren) | nein | fertig, Beispiel byte-genau gleich | `aufgaben/04-six-on-the-rail/` (Outputs: `daten/out_*.txt`) |
| 5 | Cloudflight Nanofabricator, Level 5 "The Picky Arm" (Muster erkennen und zählen) | nein | fertig, Beispiel byte-genau gleich | `aufgaben/05-picky-arm/` (Outputs: `daten/out_*.txt`) |
| 6 | Cloudflight Nanofabricator, Level 6 "Machine time isn't cheap" (Optimierung: kürzeste Programme, die jedes Muster K-mal erzeugen) | nein | fertig, alle Programme per Simulator (Level 5) verifiziert. Zeilen: example 65, small 344, medium 26611, large 22736, enormous 20779 (Referenz im Beispiel: 446 für example). Verfahren: je Emitter-Paar BFS-Greedy, bestes Paar | `aufgaben/06-machine-time/` (Outputs: `daten/out_*.txt`) |

Nächster Schritt: Nutzer schickt weitere Aufgaben (z. B. Level 7), Vorgehen siehe `CLAUDE.md`.

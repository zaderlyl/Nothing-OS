# Pont « ouvre l'appli » — Nothing OS

Quand tu fais `/app` dans Nothing OS et que tu choisis une application, le
noyau écrit le nom demandé dans `<partage>/.nothingos-open`. Le script
`opener.sh`, lancé sur le Mac, surveille ce fichier et ouvre la **vraie
application du Mac par-dessus** la fenêtre QEMU (`open -a`).

Pas de capture d'écran, pas d'affichage embarqué, aucune autorisation
spéciale. Asti (compagnon macOS) reste au premier plan au-dessus de tout.

## Utilisation

```
bridge/run.sh          # surveille ~/Documents/.nothingos-open
```

(ou `bridge/run.sh /autre/dossier` si `make run` partage un autre dossier).

Puis, dans un autre terminal : `make run` → dans l'OS : `/app`, choisis
Discord / VS Code / Affinity. La vraie appli s'ouvre par-dessus. `Cmd+Tab`
pour revenir à Nothing OS.

## Applications reconnues

| mot écrit par l'OS | `open -a`             |
|--------------------|----------------------|
| `discord`          | Discord              |
| `vscode`           | Visual Studio Code   |
| `affinity`         | Affinity             |

Pour en ajouter une : une ligne dans le `case` de `opener.sh`, et un
`Item` dans `ITEMS` (`src/apps.rs`).

# Pont « bureau distant » — Discord (et plus tard /web, vidéo)

`discord-bridge` capture la vraie fenêtre **Discord** du Mac, la réduit et
l'envoie à Nothing OS par le **partage 9p** ; les clics/frappes repartent
vers le Discord du Mac. Résultat : le vrai Discord, affiché et pilotable
depuis l'OS (`/app discord`).

## Utilisation

1. Ouvre **Discord** sur le Mac (fenêtre visible, pas réduite).
2. Depuis le Terminal, à la racine du dépôt :

   ```
   bridge/build.sh      # une fois
   bridge/run.sh        # lance + affiche le journal
   ```

   (`run.sh` prend `~/Documents` par défaut, soit le dossier partagé par
   `make run` ; sinon `bridge/run.sh /autre/dossier`.)

3. **Autorisations** — lancé depuis le Terminal, le pont utilise
   *l'autorisation du Terminal*. Si le journal dit « autorisation
   requise » :
   - Réglages ▸ Confidentialité et sécurité ▸ **Enregistrement de
     l'écran** ▸ coche **Terminal** (ou iTerm…).
   - idem ▸ **Accessibilité** ▸ coche **Terminal** (souris/clavier).
   Le pont réessaie tout seul toutes les 3 s — **pas besoin de relancer**.

   Journal attendu :
   ```
   [bridge] acces ecran OK
   [bridge] flux demarre — fenetre (1496.0, 895.0)
   [bridge] 1re image OK — 116 couleurs
   ```

4. Autre terminal : `make run` → dans Nothing OS : `/app discord`. Si le
   pont tourne, c'est le vrai Discord ; sinon la maquette.

Journal du pont : `~/Library/Logs/nothing-bridge.log`
Diagnostic noyau : lignes `[remote]` dans le terminal de `make run`.

## Protocole (fichiers dans `<partage>/.nothingos-bridge/`)

- `frame.bin` : `"NOSF"` + seq(u32) + w(u16) + h(u16) + full(u8) +
  ntiles(u16) + ntiles × { tx(u16), ty(u16), 32×32 octets indexés }.
  Écrit atomiquement (tmp + rename). Indices = cube 180 couleurs du
  noyau (`src/image.rs`).
- `input.bin` : `"NOSI"` + iseq(u32) + count(u16) + events.
  Events : `M x y` / `D|U btn x y` / `W dy x y` / `K down ascii` / `F`
  (demande de trame complète).

## Limites

- ~10 i/s, réduit à 960×576 → chat OK, pas un appel vidéo fluide.
- L'audio Discord n'est pas encore repris (à brancher sur `src/ac97.rs`).

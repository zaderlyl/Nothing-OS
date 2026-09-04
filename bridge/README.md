# Pont « bureau distant » — Discord (et plus tard /web, vidéo)

`discord-bridge` capture la vraie fenêtre **Discord** du Mac, la réduit et
l'envoie à Nothing OS par le **partage 9p** ; les clics/frappes repartent
vers le Discord du Mac. Résultat : le vrai Discord, affiché et pilotable
depuis l'OS (`/app discord`).

## Compilation

```
swiftc -O bridge/discord-bridge.swift -o bridge/discord-bridge
```

## Utilisation

1. Ouvre **Discord** sur le Mac (fenêtre visible, pas réduite).
2. Lance le pont sur le **même dossier** que celui partagé par QEMU
   (par défaut `~/Documents`) :

   ```
   bridge/discord-bridge ~/Documents
   ```

3. Autorise, à la première exécution :
   - **Enregistrement de l'écran** (capture) — Réglages ▸ Confidentialité
     et sécurité ▸ Enregistrement de l'écran ▸ ajouter le Terminal.
   - **Accessibilité** (souris/clavier) — même endroit ▸ Accessibilité.
   Relance le pont après avoir coché.

4. Dans Nothing OS : `/app discord`. Si le pont tourne, c'est le vrai
   Discord ; sinon la maquette s'affiche.

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

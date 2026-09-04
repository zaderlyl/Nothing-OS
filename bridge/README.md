# Pont « bureau distant » — Discord (et plus tard /web, vidéo)

`discord-bridge` capture la vraie fenêtre **Discord** du Mac, la réduit et
l'envoie à Nothing OS par le **partage 9p** ; les clics/frappes repartent
vers le Discord du Mac. Résultat : le vrai Discord, affiché et pilotable
depuis l'OS (`/app discord`).

## Compilation

```
bridge/build.sh
```

Produit `bridge/NothingBridge.app`. **ScreenCaptureKit exige un bundle**
pour capturer l'écran de façon fiable ; l'autorisation se rattache alors
au bundle et survit aux recompilations.

## Utilisation

1. Ouvre **Discord** sur le Mac (fenêtre visible, pas réduite).
2. Lance le pont sur le **même dossier** que celui partagé par QEMU
   (par défaut `~/Documents`) :

   ```
   open bridge/NothingBridge.app --args "$HOME/Documents"
   ```

3. **Autorisations** (à la première exécution une boîte de dialogue
   apparaît ; sinon va la cocher à la main) :
   - Réglages ▸ Confidentialité et sécurité ▸ **Enregistrement de
     l'écran** ▸ coche **NothingBridge**.
   - Réglages ▸ Confidentialité et sécurité ▸ **Accessibilité** ▸ coche
     **NothingBridge** (pour la souris/clavier).
   Puis **relance** : `open bridge/NothingBridge.app --args "$HOME/Documents"`.

4. `make run` → dans Nothing OS : `/app discord`. Si le pont tourne,
   c'est le vrai Discord ; sinon la maquette s'affiche.

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

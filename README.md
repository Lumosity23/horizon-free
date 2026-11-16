# Horizon + Tailcat

**Horizon with Tailcat for Minecraft built in.**

[Horizon](https://github.com/horizon-rust/horizon) is Minecraft: Java Edition 26.3 rebuilt in Rust. This
fork adds [Tailcat for Minecraft](https://github.com/tailscale/tailcat-for-minecraft): share your world
with friends over an encrypted WireGuard tunnel, with no port to open on your router and no VPN to install.

## How to play with friends

1. Open a world, press Esc, click the **Play with Friends** icon, then **Share with Tailcat**.
2. Copy the invitation (`mcl1_...`) and send it to your friends.
3. They paste it in **Direct Connection** or **Join a Friend**, in the game or in the launcher.

Friends need this version of Horizon too. Everything runs from the launcher: there is nothing to type in a
terminal.

## Downloads

Builds for Windows, macOS and Linux are on the [Releases](../../releases) page: the launcher (`launcher@...`)
and the game (`nightly`). Install the launcher, and it downloads this version of the game.

## Good to know

- Tailcat's helper (`mclink-helper`) comes with the game and starts by itself. It is a small Go program
  from Tailscale's prototype; **this is not an official Tailscale product**.
- To find a route between you and your friends it contacts Tailscale's relay servers (DERP). Nothing else of
  yours is sent.
- You need your own copy of Minecraft: Java Edition. No Mojang file is included in this repository or in the
  builds.
- Not affiliated with Mojang, Microsoft, Tailscale or the original Horizon team.

Based on [Horizon-free](https://github.com/0CYBER/horizon-free), itself a fork of
[Horizon](https://github.com/horizon-rust/horizon). The README of Horizon-free, with its notes, is
[README.horizon-free.md](README.horizon-free.md). How the Tailcat part works:
[docs/play-with-friends.md](docs/play-with-friends.md#tailcat-optional).

**Licenses.** Horizon's own work is under the [GNU LGPL v3 or later](LICENSE); code merged before that move
stays available under the [MIT licence](LICENSES/MIT.txt). The Tailcat helper in `tailcat-helper/` is
BSD-3-Clause (Tailscale Inc & contributors). See the [notices](THIRD_PARTY_NOTICES.md).

---

# Horizon + Tailcat (français)

**Horizon avec Tailcat for Minecraft intégré.**

Ce fork de [Horizon](https://github.com/horizon-rust/horizon) (Minecraft Java 26.3 réécrit en Rust) ajoute
[Tailcat for Minecraft](https://github.com/tailscale/tailcat-for-minecraft) : partage ton monde avec tes amis
par un tunnel WireGuard chiffré, sans ouvrir de port sur ta box et sans installer de VPN.

**Pour jouer avec des amis :** ouvre un monde, Échap, icône **Play with Friends**, puis **Share with
Tailcat**. Copie l'invitation `mcl1_...` et envoie-la. Tes amis la collent dans **Connexion directe** ou
**Join a Friend**. Il leur faut cette version de Horizon, et leur propre copie de Minecraft Java.

Les versions Windows, macOS et Linux sont dans [Releases](../../releases). Projet non officiel, sans lien
avec Mojang, Microsoft, Tailscale ni l'équipe d'origine de Horizon.

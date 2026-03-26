import type { LoaderGuide, NavigationItem } from "./types";

export const navigationItems: NavigationItem[] = [
  { id: "play", label: "Play", kicker: "起動" },
  { id: "mods", label: "My Mods", kicker: "管理" },
  { id: "discover", label: "Discover", kicker: "検索" },
  { id: "loaders", label: "Loader", kicker: "導入" },
  { id: "settings", label: "Settings", kicker: "環境" },
];

export const loaderGuides: LoaderGuide[] = [
  {
    id: "fabric",
    name: "Fabric",
    kicker: "自動導入",
    description: "このアプリから Minecraft バージョンを選び、Fabric の導入と起動構成の作成まで完了します。",
    detail: "軽量系・UI系・パフォーマンス系 Mod を中心に始めるなら最も扱いやすい構成です。",
    url: "https://fabricmc.net/use/installer/",
    automation: "full",
  },
  {
    id: "forge",
    name: "Forge",
    kicker: "自動導入",
    description: "大型 Modpack や古い定番構成で使われることが多いローダーです。",
    detail: "公式 Installer を staging 経由で実行し、Version と起動構成をこのアプリ側で整えます。",
    url: "https://files.minecraftforge.net/net/minecraftforge/forge/",
    automation: "full",
  },
  {
    id: "neoforge",
    name: "NeoForge",
    kicker: "自動導入",
    description: "新しめのコンテンツ系 Mod 環境を組むときに選びやすいローダーです。",
    detail: "公式 Installer を使いながら、Version の追加と起動構成の作成まで自動で進めます。",
    url: "https://neoforged.net/",
    automation: "full",
  },
  {
    id: "quilt",
    name: "Quilt",
    kicker: "自動導入",
    description: "Fabric 系の流れを汲むローダーで、一部のコミュニティではこちらが選ばれます。",
    detail: "Universal installer を使って Quilt Loader と起動構成をまとめて追加します。",
    url: "https://quiltmc.org/en/install/client/",
    automation: "full",
  },
];

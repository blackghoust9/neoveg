#!/usr/bin/env python3
"""Sync Milestone 6 translation keys across all 7 catalogs in Assets/i18n/strings/"""

import json
import pathlib
import re

CATALOGS = ["de-DE", "en-US", "es-ES", "fr-FR", "ja-JP", "ru-RU", "zh-CN"]
BASE_DIR = pathlib.Path("Assets/i18n/strings")

M6_TRANSLATIONS = {
    # Feature 35 (L/J Cut)
    "Corte em L": {
        "en-US": "L-Cut",
        "de-DE": "L-Schnitt",
        "es-ES": "Corte en L",
        "fr-FR": "Cut en L",
        "ja-JP": "Lカット",
        "ru-RU": "L-образный монтаж",
        "zh-CN": "L形剪辑",
    },
    "Corte em J": {
        "en-US": "J-Cut",
        "de-DE": "J-Schnitt",
        "es-ES": "Corte en J",
        "fr-FR": "Cut en J",
        "ja-JP": "Jカット",
        "ru-RU": "J-образный монтаж",
        "zh-CN": "J形剪辑",
    },
    "Edição dividida (L/J)": {
        "en-US": "Split edit (L/J)",
        "de-DE": "Split-Bearbeitung (L/J)",
        "es-ES": "Edición dividida (L/J)",
        "fr-FR": "Édition divisée (L/J)",
        "ja-JP": "スプリット編集 (L/J)",
        "ru-RU": "Раздельный монтаж (L/J)",
        "zh-CN": "拆分编辑 (L/J)",
    },
    "Aparar áudio independentemente": {
        "en-US": "Trim audio independently",
        "de-DE": "Audio unabhängig trimmen",
        "es-ES": "Recortar audio independientemente",
        "fr-FR": "Raccorder l'audio indépendamment",
        "ja-JP": "音声を個別にトリム",
        "ru-RU": "Подрезать аудио независимо",
        "zh-CN": "独立修剪音频",
    },
    "Aparar vídeo independentemente": {
        "en-US": "Trim video independently",
        "de-DE": "Video unabhängig trimmen",
        "es-ES": "Recortar video independientemente",
        "fr-FR": "Raccorder la vidéo indépendamment",
        "ja-JP": "映像を個別にトリム",
        "ru-RU": "Подрезать видео независимо",
        "zh-CN": "独立修剪视频",
    },
    # Feature 36 (Simultaneous Fades)
    "Fades simultâneos de áudio e vídeo": {
        "en-US": "Simultaneous audio and video fades",
        "de-DE": "Gleichzeitige Audio- und Video-Fades",
        "es-ES": "Fades simultáneos de audio y vídeo",
        "fr-FR": "Fondus audio et vidéo simultanés",
        "ja-JP": "音声と映像の同時フェード",
        "ru-RU": "Одновременные аудио- и видео-фейды",
        "zh-CN": "音视频同步淡入淡出",
    },
    "Micro-fade rápido de áudio": {
        "en-US": "Audio micro-quickfade",
        "de-DE": "Audio-Mikro-Quickfade",
        "es-ES": "Micro-fade rápido de audio",
        "fr-FR": "Micro-fondu rapide audio",
        "ja-JP": "音声マイクロクイックフェード",
        "ru-RU": "Быстрый микрофейд аудио",
        "zh-CN": "音频微型快速淡入淡出",
    },
    "Vincular fades A/V": {
        "en-US": "Link A/V fades",
        "de-DE": "A/V-Fades verknüpfen",
        "es-ES": "Vincular fades A/V",
        "fr-FR": "Lier les fondus A/V",
        "ja-JP": "A/Vフェードをリンク",
        "ru-RU": "Связать фейды A/V",
        "zh-CN": "链接音视频淡入淡出",
    },
    # Feature 37 (Sync Links)
    "Vínculo de sincronização": {
        "en-US": "Sync link",
        "de-DE": "Sync-Link",
        "es-ES": "Vínculo de sincronización",
        "fr-FR": "Lien de synchronisation",
        "ja-JP": "同期リンク",
        "ru-RU": "Связь синхронизации",
        "zh-CN": "同步链接",
    },
    "Criar vínculo de sincronização": {
        "en-US": "Create sync link",
        "de-DE": "Sync-Link erstellen",
        "es-ES": "Crear vínculo de sincronización",
        "fr-FR": "Créer un lien de synchronisation",
        "ja-JP": "同期リンクを作成",
        "ru-RU": "Создать связь синхронизации",
        "zh-CN": "创建同步链接",
    },
    "Remover vínculo de sincronização": {
        "en-US": "Remove sync link",
        "de-DE": "Sync-Link entfernen",
        "es-ES": "Eliminar vínculo de sincronización",
        "fr-FR": "Supprimer le lien de synchronisation",
        "ja-JP": "同期リンクを削除",
        "ru-RU": "Удалить связь синхронизации",
        "zh-CN": "移除同步链接",
    },
    "Fora de sincronia: {} quadros": {
        "en-US": "Out of sync: {} frames",
        "de-DE": "Asynchron: {} Frames",
        "es-ES": "Fuera de sincronía: {} fotogramas",
        "fr-FR": "Désynchronisé: {} images",
        "ja-JP": "非同期: {}フレーム",
        "ru-RU": "Рассинхронизация: {} кадров",
        "zh-CN": "不同步: {} 帧",
    },
    "Sincronizar por deslocamento": {
        "en-US": "Synchronize by moving",
        "de-DE": "Synchronisieren durch Verschieben",
        "es-ES": "Sincronizar moviendo",
        "fr-FR": "Synchroniser par déplacement",
        "ja-JP": "移動して同期",
        "ru-RU": "Синхронизировать перемещением",
        "zh-CN": "通过移动进行同步",
    },
    "Sincronizar por deslizamento": {
        "en-US": "Synchronize by slipping",
        "de-DE": "Synchronisieren durch Slippen",
        "es-ES": "Sincronizar deslizando",
        "fr-FR": "Synchroniser par glissement",
        "ja-JP": "スリップして同期",
        "ru-RU": "Синхронизировать скольжением",
        "zh-CN": "通过滑动内容进行同步",
    },
    "Restaurar sincronismo A/V": {
        "en-US": "Restore A/V sync",
        "de-DE": "A/V-Sync wiederherstellen",
        "es-ES": "Restaurar sincronización A/V",
        "fr-FR": "Restaurer la synchronisation A/V",
        "ja-JP": "A/V同期を復元",
        "ru-RU": "Восстановить A/V синхронизацию",
        "zh-CN": "恢复音视频同步",
    },
    # Feature 38 (Multicam Sync)
    "Sincronizar multicâmera por áudio": {
        "en-US": "Sync multicamera by audio",
        "de-DE": "Multikamera nach Audio synchronisieren",
        "es-ES": "Sincronizar multicámara por audio",
        "fr-FR": "Synchroniser la multicaméra par audio",
        "ja-JP": "マルチカメラを音声で同期",
        "ru-RU": "Синхронизировать мультикамеру по аудио",
        "zh-CN": "按音频同步多机位",
    },
    "Sincronizar multicâmera por timecode": {
        "en-US": "Sync multicamera by timecode",
        "de-DE": "Multikamera nach Timecode synchronisieren",
        "es-ES": "Sincronizar multicámara por código de tiempo",
        "fr-FR": "Synchroniser la multicaméra par timecode",
        "ja-JP": "マルチカメラをタイムコードで同期",
        "ru-RU": "Синхронизировать мультикамеру по таймкоду",
        "zh-CN": "按时间码同步多机位",
    },
    "Alinhamento multicâmera concluído: {} ângulos": {
        "en-US": "Multicamera alignment complete: {} angles",
        "de-DE": "Multikamera-Ausrichtung abgeschlossen: {} Winkel",
        "es-ES": "Alineación multicámara completada: {} ángulos",
        "fr-FR": "Alignement multicaméra terminé: {} angles",
        "ja-JP": "マルチカメラ配置完了: {}アングル",
        "ru-RU": "Выравнивание мультикамеры завершено: {} ракурсов",
        "zh-CN": "多机位对齐完成: {} 个机位",
    },
    "Falha no alinhamento de áudio multicâmera": {
        "en-US": "Multicamera audio alignment failed",
        "de-DE": "Multikamera-Audioausrichtung fehlgeschlagen",
        "es-ES": "Error en la alineación de audio multicámara",
        "fr-FR": "Échec de l'alignement audio multicaméra",
        "ja-JP": "マルチカメラ音声配置に失敗しました",
        "ru-RU": "Сбой выравнивания аудио мультикамеры",
        "zh-CN": "多机位音频对齐失败",
    },
    # Feature 39 (Multicam Track & Angle Switching)
    "Criar faixa multicâmera": {
        "en-US": "Create multicamera track",
        "de-DE": "Multikamera-Spur erstellen",
        "es-ES": "Crear pista multicámara",
        "fr-FR": "Créer une piste multicaméra",
        "ja-JP": "マルチカメラトラックを作成",
        "ru-RU": "Создать мультикамерную дорожку",
        "zh-CN": "创建多机位轨道",
    },
    "Alternar para ângulo {}": {
        "en-US": "Switch to angle {}",
        "de-DE": "Zu Winkel {} wechseln",
        "es-ES": "Cambiar a ángulo {}",
        "fr-FR": "Passer à l'angle {}",
        "ja-JP": "アングル{}に切り替え",
        "ru-RU": "Переключить на ракурс {}",
        "zh-CN": "切换到机位 {}",
    },
    "Próximo ângulo de câmera": {
        "en-US": "Next camera angle",
        "de-DE": "Nächster Kamerawinkel",
        "es-ES": "Siguiente ángulo de cámara",
        "fr-FR": "Angle de caméra suivant",
        "ja-JP": "次のカメラアングル",
        "ru-RU": "Следующий ракурс камеры",
        "zh-CN": "下一个摄像机机位",
    },
    "Ângulo de câmera anterior": {
        "en-US": "Previous camera angle",
        "de-DE": "Vorheriger Kamerawinkel",
        "es-ES": "Ángulo de cámara anterior",
        "fr-FR": "Angle de caméra précédent",
        "ja-JP": "前のカメラアングル",
        "ru-RU": "Предыдущий ракурс камеры",
        "zh-CN": "上一个摄像机机位",
    },
    "Edição multicâmera ao vivo": {
        "en-US": "Live multicamera editing",
        "de-DE": "Live-Multikamera-Bearbeitung",
        "es-ES": "Edición multicámara en directo",
        "fr-FR": "Édition multicaméra en direct",
        "ja-JP": "ライブマルチカメラ編集",
        "ru-RU": "Живой мультикамерный монтаж",
        "zh-CN": "实时多机位编辑",
    },
    "Expandir faixa multicâmera": {
        "en-US": "Expand multicamera track",
        "de-DE": "Multikamera-Spur erweitern",
        "es-ES": "Expandir pista multicámara",
        "fr-FR": "Développer la piste multicaméra",
        "ja-JP": "マルチカメラトラックを展開",
        "ru-RU": "Развернуть мультикамерную дорожку",
        "zh-CN": "展开多机位轨道",
    },
    "Grade multicâmera": {
        "en-US": "Multicamera grid",
        "de-DE": "Multikamera-Raster",
        "es-ES": "Cuadrícula multicámara",
        "fr-FR": "Grille multicaméra",
        "ja-JP": "マルチカメラグリッド",
        "ru-RU": "Сетка мультикамеры",
        "zh-CN": "多机位网格",
    },
    # Feature 40 (Takes System)
    "Deslizar todos os takes": {
        "en-US": "Slip all takes",
        "de-DE": "Alle Takes slippen",
        "es-ES": "Deslizar todas las tomas",
        "fr-FR": "Glisser toutes les prises",
        "ja-JP": "すべてのテイクをスリップ",
        "ru-RU": "Скользить все дубли",
        "zh-CN": "滑动所有镜头",
    },
    "Renomear take ativo": {
        "en-US": "Rename active take",
        "de-DE": "Aktiven Take umbenennen",
        "es-ES": "Renombrar toma activa",
        "fr-FR": "Renommer la prise active",
        "ja-JP": "アクティブなテイクの名前を変更",
        "ru-RU": "Переименовать активный дубль",
        "zh-CN": "重命名活动镜头",
    },
    "Escolher take ativo": {
        "en-US": "Choose active take",
        "de-DE": "Aktiven Take auswählen",
        "es-ES": "Elegir toma activa",
        "fr-FR": "Choisir la prise active",
        "ja-JP": "アクティブなテイクを選択",
        "ru-RU": "Выбрать активный дубль",
        "zh-CN": "选择活动镜头",
    },
    "Take {} de {}": {
        "en-US": "Take {} of {}",
        "de-DE": "Take {} von {}",
        "es-ES": "Toma {} de {}",
        "fr-FR": "Prise {} sur {}",
        "ja-JP": "テイク {} / {}",
        "ru-RU": "Дубль {} из {}",
        "zh-CN": "镜头 {} / {}",
    },
    "Nome do take": {
        "en-US": "Take name",
        "de-DE": "Take-Name",
        "es-ES": "Nombre de toma",
        "fr-FR": "Nom de prise",
        "ja-JP": "テイク名",
        "ru-RU": "Имя дубля",
        "zh-CN": "镜头名称",
    },
}

ph_pattern = re.compile(r"\{[^{}]*\}")

def sync():
    for cat in CATALOGS:
        path = BASE_DIR / f"{cat}.json"
        with open(path, "r", encoding="utf-8") as f:
            data = json.load(f)
        
        added = 0
        for pt_key, translations in M6_TRANSLATIONS.items():
            trans = translations.get(cat)
            if not trans:
                raise ValueError(f"Missing translation for {cat} on key: {pt_key}")
            
            # Check placeholder consistency
            pt_phs = len(ph_pattern.findall(pt_key))
            tr_phs = len(ph_pattern.findall(trans))
            if pt_phs != tr_phs:
                raise ValueError(f"Placeholder mismatch for {cat} on key '{pt_key}': {pt_phs} vs {tr_phs}")
            
            if pt_key not in data:
                data[pt_key] = trans
                added += 1
            else:
                data[pt_key] = trans

        with open(path, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, indent=2)
            f.write("\n")
        print(f"Catalog {cat}: updated, now has {len(data)} keys (+{added} added)")

if __name__ == "__main__":
    sync()

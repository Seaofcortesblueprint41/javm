export function formatDuration(seconds: number): string {
    if (isNaN(seconds) || seconds < 0) return "00:00";

    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);

    const format = (n: number) => n.toString().padStart(2, '0');

    if (h > 0) {
        return `${format(h)}:${format(m)}:${format(s)}`;
    }
    return `${format(m)}:${format(s)}`;
}

export function formatRating(rating: number): string {
    return rating.toFixed(1);
}

export function formatFileSize(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

export function formatSpeed(bytesPerSecond: number): string {
    return `${formatFileSize(bytesPerSecond)}/s`;
}

export function formatProgress(progress: number): string {
    // 使用 Math.floor 向下取整，避免 99.94% 显示为 100%
    return `${Math.floor(progress)}%`;
}

export function formatTime(timestamp: number | string | Date): string {
    const date = new Date(timestamp);
    return date.toLocaleTimeString('zh-CN', { hour12: false });
}

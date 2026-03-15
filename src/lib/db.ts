import Database from '@tauri-apps/plugin-sql'

let db: Database | null = null

export async function getDb() {
    if (!db) {
        db = await Database.load('sqlite:videos.db')
    }
    return db
}

export async function query<T>(sql: string, params?: unknown[]): Promise<T> {
    const db = await getDb()
    return db.select<T>(sql, params)
}

export async function execute(sql: string, params?: unknown[]): Promise<void> {
    const db = await getDb()
    await db.execute(sql, params)
}

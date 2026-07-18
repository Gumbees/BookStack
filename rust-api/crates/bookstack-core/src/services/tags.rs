use sqlx::PgPool;

use crate::models::Tag;
use crate::Result;

pub async fn get_for(db: &PgPool, entity_type: &str, entity_id: i64) -> Result<Vec<Tag>> {
    Ok(sqlx::query_as::<_, Tag>(
        r#"SELECT * FROM tags WHERE entity_type = $1 AND entity_id = $2 ORDER BY "order", id"#,
    )
    .bind(entity_type)
    .bind(entity_id)
    .fetch_all(db)
    .await?)
}

pub async fn set_for(db: &PgPool, entity_type: &str, entity_id: i64, tags: &[Tag]) -> Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM tags WHERE entity_type = $1 AND entity_id = $2")
        .bind(entity_type)
        .bind(entity_id)
        .execute(&mut *tx)
        .await?;
    for (i, tag) in tags.iter().enumerate() {
        sqlx::query(
            r#"INSERT INTO tags (entity_type, entity_id, name, value, "order") VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(entity_type)
        .bind(entity_id)
        .bind(&tag.name)
        .bind(&tag.value)
        .bind(i as i32)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn delete_for(db: &PgPool, entity_type: &str, entity_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM tags WHERE entity_type = $1 AND entity_id = $2")
        .bind(entity_type)
        .bind(entity_id)
        .execute(db)
        .await?;
    Ok(())
}

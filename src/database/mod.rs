mod aggregates;
mod changelog;
pub mod history;
pub(crate) mod impls;
pub(crate) mod models;

use color_eyre::{Result, eyre::Context};
use futures::future::BoxFuture;
use futures::stream::BoxStream;
use sqlx::postgres::{PgPoolOptions, PgQueryResult, PgRow, PgStatement, PgTypeInfo};
use sqlx::{Describe, Error as SqlxError, Execute, PgPool, SqlStr};
use sqlx::{Either, Executor, Postgres, Transaction, pool::PoolConnection};

#[derive(Debug)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(uri: &str) -> Result<Self> {
        tracing::debug!("Opening Postgres connection pool");
        let pool = PgPoolOptions::new().connect(uri).await?;
        tracing::info!("Postgres connection pool ready");

        Ok(Self { pool })
    }

    pub async fn acquire(&self) -> Result<PoolConnection<Postgres>> {
        tracing::trace!("Acquiring Postgres connection from pool");
        self.pool
            .acquire()
            .await
            .wrap_err("Failed to acquire a Postgres connection")
    }

    pub async fn begin(&self) -> Result<Transaction<'static, Postgres>> {
        tracing::trace!("Starting Postgres transaction");
        self.pool
            .begin()
            .await
            .wrap_err("Failed to start a Postgres transaction")
    }

    /// Disables chunkwise aggregation for the current query
    ///
    /// Massively speeds up some aggregate-style functions that don't touch dates
    pub async fn begin_with_chunkwise_aggregation_disabled(
        &self,
    ) -> Result<Transaction<'static, Postgres>> {
        tracing::trace!(
            "Starting Postgres transaction with timescaledb.enable_chunkwise_aggregation = off"
        );
        let mut trans = self.pool.begin().await.wrap_err(
            "Failed to start a Postgres transaction with disabled chunkwise aggregation",
        )?;

        sqlx::query("SET LOCAL timescaledb.enable_chunkwise_aggregation = off")
            .execute(&mut *trans)
            .await?;

        Ok(trans)
    }

    pub async fn migrate(&self) -> Result<()> {
        tracing::trace!("Running migrations");
        let pool = &self.pool;
        sqlx::migrate!("./migrations").run(pool).await?;

        Ok(())
    }
}

impl<'d, 'p> Executor<'p> for &'d Database {
    type Database = Postgres;

    #[inline]
    fn fetch_many<'e, 'q: 'e, E: 'q>(
        self,
        query: E,
    ) -> BoxStream<'e, std::result::Result<Either<PgQueryResult, PgRow>, SqlxError>>
    where
        'p: 'e,
        E: Execute<'q, Self::Database>,
    {
        <&PgPool as Executor<'p>>::fetch_many(&self.pool, query)
    }

    #[inline]
    fn fetch_optional<'e, 'q: 'e, E: 'q>(
        self,
        query: E,
    ) -> BoxFuture<'e, Result<Option<PgRow>, SqlxError>>
    where
        'p: 'e,
        E: Execute<'q, Self::Database>,
    {
        <&PgPool as Executor<'p>>::fetch_optional(&self.pool, query)
    }

    #[inline]
    fn prepare_with<'e>(
        self,
        sql: SqlStr,
        parameters: &'e [PgTypeInfo],
    ) -> BoxFuture<'e, Result<PgStatement, SqlxError>>
    where
        'p: 'e,
    {
        <&PgPool as Executor<'p>>::prepare_with(&self.pool, sql, parameters)
    }

    #[inline]
    fn describe<'e>(self, sql: SqlStr) -> BoxFuture<'e, Result<Describe<Self::Database>, SqlxError>>
    where
        'p: 'e,
    {
        <&PgPool as Executor<'p>>::describe(&self.pool, sql)
    }
}

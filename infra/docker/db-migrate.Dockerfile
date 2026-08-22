FROM postgres:16-alpine
COPY infra/db/sql /sql
COPY infra/docker/db-migrate.sh /migrate.sh
RUN chmod +x /migrate.sh
ENTRYPOINT ["/migrate.sh"]

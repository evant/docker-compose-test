# docker-compose-test

A helper to run integration tests with docker-compose.

## Usage

In addition to your normal `docker-compose.yml`, create a `docker-compose.integration-tests.yml` to define
your services to run tests on. For example, say if you have the below setup:

```yaml
# docker-compose.yml
services:
  postgres:
    image: docker.io/postgres:13.1-alpine
    ports:
      - "0.0.0.0:5432:5432"
    environment:
      POSTGRES_PASSWORD: password
  server:
    build: server
    command: npm run start
    ports:
      - "0.0.0.0:3000:3000"
    volumes:
      - "./src:/app" 
    depends_on:
      - postgres
```

then you'd define:

```yaml
# docker-compose.integration-tests.yml
services:
  server:
    command: npm test
```

Then you can run `docker-compose-test` and it'll spin up your database and run your tests!

If you have multiple services this will run all of them by default. You can also pass in the services you want run.

```yaml
services:
  server:
    command: npm test
  web:
    command: npm test
```

`docker-compose-test web` will only run web.

## Configuration

If you don't want to use the default file names, you can pass them explicitly. Note, it's expected that your test 
service is the last file.

`docker-compose-test -f container-compose.yml -f test-container-compose.yml`

You can override the default docker & docker-compose binaries by using the `$DOCKER` and `$DOCKER_COMPOSE` env variables
respectively.

```shell
export DOCKER=podman
export DOCKER_COMPOSE=podman-compose
docker-compose-test
```
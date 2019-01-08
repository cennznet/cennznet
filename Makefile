DIR := ${CURDIR}

#if make is typed with no further arguments, then show a list of available targets
default:
	@awk -F\: '/^[a-z_]+:/ && !/default/ {printf "- %-20s %s\n", $$1, $$2}' Makefile

help:
	@echo ""
	@echo "make up: start all containers"
	@echo "make stop [CONTAINER]: stop [CONTAINER]"
	@echo "make logs [CONTAINER]: get logs"
	@echo "make down: do a docker-compose down -v"
	@echo "make start [CONTAINER]: start [CONTAINER]"
	@echo ""

build:
	@docker build $(filter-out $@,$(MAKECMDGOALS))

build-up:
	docker-compose build && docker-compose up -d $(filter-out $@,$(MAKECMDGOALS))

up:
	docker-compose up -d $(filter-out $@,$(MAKECMDGOALS))

pull:
	docker-compose pull --parallel

down:
	docker-compose down $(filter-out $@,$(MAKECMDGOALS))

start:
	docker-compose start $(filter-out $@,$(MAKECMDGOALS))

stop:
	docker-compose stop $(filter-out $@,$(MAKECMDGOALS))

restart:
	docker-compose restart $(filter-out $@,$(MAKECMDGOALS))

logs:
	docker-compose logs -f --tail=200 $(filter-out $@,$(MAKECMDGOALS))

prune:
	@echo "pruning some old containers and images"
	docker container prune --filter "until=336h"
	docker image prune -a

%:
	@:

# Avro Enum Jar Scanner

This is tiny project dedicated to scanning a jar-file in order to retrieve information about `enum`-s of interest. 

In particular, following information is analyzed and output:
- fully qualified class name of an `enum`
- whether or not it is annotated with `@AvroGenerated` annotation
- source: root jar, or one of the [nested jars](https://docs.spring.io/spring-boot/docs/current/reference/htmlsingle/#appendix.executable-jar.nested-jars)
- all enum members

## Some background information: 
This project is conceived in an organization that relies heavily on `Avro` format along with
`.avsc` schemas and code generation. Obviously, some of the microservices use "proper" enums, while others just compile 
"foreign" enums from `.avsc` definitions. 

And during deployment this sometimes creates havoc, if proper care is not being applied. It might so happen that a service
might get deployed, that _publishes_ certain event, will contain a _newer_ enum definition (including enum members that consuming services do not yet know of). This might cause stops in event consumption that can only be fixed by an urgent patching and deployment of a consumer service.

In order to mitigate this problem, current utility might prove to be useful

## How to use it

Let's say you have a Spring Boot JAR file lying around.

Then you can run the app as follows: 

`avro-enum-jar-scanner[.exe] --jarfile path/to/my.jar --class-name-regex ^com/example/.*$`

or, using short command names, as 

`avro-enum-jar-scanner[.exe] -j path/to/my.jar -c ^com/example/.*$`

If all goes well, it should produce JSON output similar to this one: 

```json
[
  {
    "class_name": "org/example/avro/OAuthStatus",
    "members": [
      "PENDING",
      "ACTIVE",
      "DENIED",
      "EXPIRED",
      "REVOKED"
    ],
    "avro_generated": true,
    "source": "root"
  },
  {
    "class_name": "org/example/avro/ToDoStatus",
    "members": [
      "HIDDEN",
      "ACTIONABLE",
      "DONE",
      "ARCHIVED",
      "DELETED"
    ],
    "avro_generated": true,
    "source": "root"
  },
  {
    "class_name": "org/example/demo/Fruit",
    "members": [
      "APPLE",
      "BANANA",
      "CHOKEBERRY"
    ],
    "avro_generated": false,
    "source": "root"
  },
  {
    "class_name": "org/example/demo/Vehicle",
    "members": [
      "CAR",
      "BICYCLE",
      "BUS",
      "TRAM",
      "SCOOTER"
    ],
    "avro_generated": false,
    "source": "root"
  }
]
```
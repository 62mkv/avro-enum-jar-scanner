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

Let's compile the project from [here](https://github.com/62mkv/sb-avro-enum-demo), using `gradlew bootJar`, and then apply command as

```
avro-enum-jar-scanner -j path\sb-avro-enum\server\build\libs\server-1.0-SNAPSHOT.jar -c ^org/example/.*$
```

If all goes well, it should produce JSON output similar to this one: 

```json
[
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
  },
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
    "class_name": "org/example/avro/AuthStatus",
    "members": [
      "PENDING",
      "ACTIVE",
      "DENIED",
      "EXPIRED",
      "REVOKED"
    ],
    "avro_generated": true,
    "source": "BOOT-INF/lib/client-1.0.0.jar"
  },
  {
    "class_name": "org/example/demo/Book",
    "members": [
      "JOURNAL",
      "MAGAZINE",
      "NOTEPAD",
      "DIARY"
    ],
    "avro_generated": false,
    "source": "BOOT-INF/lib/client-1.0.0.jar"
  },
  {
    "class_name": "org/example/demo/Magazine",
    "members": [
      "TM",
      "AJALUGU",
      "INIMENE",
      "MARIE"
    ],
    "avro_generated": false,
    "source": "BOOT-INF/lib/other-client-1.0.0.jar"
  }
]
```

Plus it will produce this output in `stderr`: 
```
Already scanned org/example/avro/OAuthStatus
Already scanned org/example/avro/ToDoStatus
Already scanned org/example/demo/Fruit
Already scanned org/example/demo/Vehicle
Already scanned org/example/demo/Book
Already scanned org/example/demo/Fruit
Already scanned org/example/demo/Vehicle
```

Here, we see that: 
1) files from the `BOOT-INF\classes` take precedence over "libraries", so, when class with same FQCN is observed from the _nested JAR_, it is ignored and warning is printed out.
2) order of dependencies _IS RESPECTED_ (see explanation [here](https://docs.spring.io/spring-boot/docs/current/reference/htmlsingle/#appendix.executable-jar.nested-jars.classpath-index)), in other words, when enum with same FQCN was observed already in `BOOT-INF/classes` or other visited dependency, it is ignored and warning is printed out.
3) "source" field for the observed enum indicates, where the enum is scanned from: `root` means `BOOT-INF/classes` i.e. application's own code, while anything else (full path inside a Boot JAR archive) indicates library's nested JAR. This might be helpful for debugging purposes. 
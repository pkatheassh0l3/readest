# smbj y BouncyCastle resuelven clases por reflexión (algoritmos de firma y
# cifrado SMB): sin estas reglas, una compilación con minify los borraría y la
# conexión fallaría solo en release, que es el peor momento para enterarse.
-keep class com.hierynomus.** { *; }
-keep class org.bouncycastle.** { *; }
-dontwarn org.slf4j.**
-dontwarn org.bouncycastle.**

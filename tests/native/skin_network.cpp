#include "kog_skin_network.h"
#include <QtCore/QCoreApplication>
#include <QtCore/QFile>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <future>
#include <iostream>
#include <stdexcept>

int main(int argc, char **argv)
{
    QCoreApplication application(argc, argv);
    auto worker = std::async(std::launch::async, [argc, argv] {
        for (const auto *url : {"http://archive.org/metadata/test", "https://example.org/",
                               "https://archive.org.evil.example/", "file:///etc/passwd"}) {
            bool blocked = false;
            try { kogFetchSkinUrl(url, 1024); } catch (const std::exception &) { blocked = true; }
            if (!blocked) throw std::runtime_error("Invalid URL was allowed");
        }
        if (argc > 1) {
            const auto meta = kogFetchSkinUrl("https://archive.org/metadata/winampskin_Winamp_Classic", 2 * 1024 * 1024);
            if (!QJsonDocument::fromJson(meta).object().contains("files"))
                throw std::runtime_error("Invalid live metadata");
            const auto skin = kogFetchSkinUrl("https://archive.org/download/winampskin_Winamp_Classic/Winamp_Classic.wsz", 32 * 1024 * 1024);
            if (!skin.startsWith("PK")) throw std::runtime_error("Invalid skin archive");
            QFile output(QString::fromLocal8Bit(argv[1]));
            if (!output.open(QIODevice::WriteOnly | QIODevice::NewOnly) || output.write(skin) != skin.size())
                throw std::runtime_error("Cannot write test skin");
            bool limited = false;
            try { kogFetchSkinUrl("https://archive.org/metadata/winampskin_Winamp_Classic", 8); }
            catch (const std::exception &) { limited = true; }
            if (!limited) throw std::runtime_error("Size cap was not enforced");
        }
    });
    try { worker.get(); }
    catch (const std::exception &e) { std::cerr << e.what() << '\n'; return 1; }
    std::cout << "Skin network checks passed\n";
}

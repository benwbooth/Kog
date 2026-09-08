#pragma once

#include <QtCore/QString>
#include <QtWidgets/QApplication>

#include <memory>

QString kogFileIconName(const QString &path);
std::unique_ptr<QApplication> kogApplicationNew();
void kogApplicationSetName(QApplication &application, const QString &name);
void kogApplicationSetVersion(QApplication &application, const QString &version);
int kogApplicationExec(QApplication &application);
void kogApplyApplicationIcon();
void kogRestoreMainWindow();

// SPDX-License-Identifier: MIT
#include "ShortcutDialog.hpp"

#include <QAction>
#include <QDialogButtonBox>
#include <QHash>
#include <QHeaderView>
#include <QKeySequenceEdit>
#include <QLabel>
#include <QLineEdit>
#include <QMessageBox>
#include <QPushButton>
#include <QTableWidget>
#include <QVBoxLayout>

#include "CommandRegistry.hpp"

#include "Dialogs.hpp"

#include "../platform/MacDialogs.hpp"

namespace mitcad {

ShortcutDialog::ShortcutDialog(CommandRegistry& registry, QWidget* parent)
    : QDialog(parent), m_registry(registry) {
  setWindowTitle(tr("Keyboard Shortcuts"));
  auto* layout = new QVBoxLayout(this);
  auto* instructions = new QLabel(tr("Click a shortcut and press the new keys. Use the clear button to remove it. "
                                    "Commands in separate contexts may share keys."));
  instructions->setWordWrap(true);
  layout->addWidget(instructions);
  if (!registry.shortcutWarnings().isEmpty()) {
    auto* warning = new QLabel(tr("Some saved shortcuts conflict and are disabled. The commands remain in menus. "
                                  "Change these bindings or restore defaults:\n%1")
                                  .arg(registry.shortcutWarnings().join(QLatin1Char('\n'))));
    warning->setWordWrap(true);
    layout->addWidget(warning);
  }
  auto* search = new QLineEdit;
  search->setObjectName(QStringLiteral("shortcutSearch"));
  search->setPlaceholderText(tr("Search commands or shortcuts"));
  search->setClearButtonEnabled(true);
  mac::styleSearchField(search);
  layout->addWidget(search);
  m_table = new QTableWidget(0, 2);
  m_table->setObjectName(QStringLiteral("shortcutTable"));
  mac::styleListTable(m_table);
  m_table->setHorizontalHeaderLabels({tr("Command"), tr("Shortcut")});
  m_table->horizontalHeader()->setSectionResizeMode(0, QHeaderView::Stretch);
  m_table->horizontalHeader()->setSectionResizeMode(1, QHeaderView::Stretch);
  m_table->verticalHeader()->hide();
  for (const auto& def : registry.commands()) {
    const int row = m_table->rowCount();
    m_table->insertRow(row);
    auto* name = new QTableWidgetItem(registry.action(def->id) != nullptr
                                          ? registry.action(def->id)->icon()
                                          : QIcon(),
                                      def->name);
    name->setData(Qt::UserRole, def->id);
    name->setFlags(name->flags() & ~Qt::ItemIsEditable);
    m_table->setItem(row, 0, name);
    auto* edit = new QKeySequenceEdit(registry.shortcut(def->id));
    edit->setMaximumSequenceLength(1);
    edit->setClearButtonEnabled(true);
    m_table->setCellWidget(row, 1, edit);
  }
  layout->addWidget(m_table);
  const auto filter = [this, search] {
    const QString needle = search->text().trimmed();
    for (int row = 0; row < m_table->rowCount(); ++row) {
      const auto* editor = static_cast<QKeySequenceEdit*>(m_table->cellWidget(row, 1));
      const bool match = needle.isEmpty() || m_table->item(row, 0)->text().contains(needle, Qt::CaseInsensitive) ||
                         editor->keySequence().toString(QKeySequence::NativeText).contains(needle, Qt::CaseInsensitive);
      m_table->setRowHidden(row, !match);
    }
  };
  connect(search, &QLineEdit::textChanged, this, filter);
  for (int row = 0; row < m_table->rowCount(); ++row) {
    connect(static_cast<QKeySequenceEdit*>(m_table->cellWidget(row, 1)), &QKeySequenceEdit::keySequenceChanged,
            this, filter);
  }

  auto* buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel |
                                       QDialogButtonBox::RestoreDefaults);
  connect(buttons, &QDialogButtonBox::accepted, this, &ShortcutDialog::accept);
  connect(buttons, &QDialogButtonBox::rejected, this, &ShortcutDialog::reject);
  connect(buttons->button(QDialogButtonBox::RestoreDefaults), &QPushButton::clicked, this,
          &ShortcutDialog::resetAll);
  layout->addWidget(buttons);
  mac::styleDialogButtons(buttons, nullptr, true);
  mac::styleDialog(this);
  resize(460, 520);
}

void ShortcutDialog::resetAll() {
  for (int row = 0; row < m_table->rowCount(); ++row) {
    const QString id = m_table->item(row, 0)->data(Qt::UserRole).toString();
    static_cast<QKeySequenceEdit*>(m_table->cellWidget(row, 1))
        ->setKeySequence(m_registry.defaultShortcut(id));
  }
}

void ShortcutDialog::accept() {
  QHash<QString, QKeySequence> proposed;
  for (int row = 0; row < m_table->rowCount(); ++row) {
    proposed.insert(m_table->item(row, 0)->data(Qt::UserRole).toString(),
                    static_cast<QKeySequenceEdit*>(m_table->cellWidget(row, 1))->keySequence());
  }
  const auto conflicts = m_registry.shortcutConflicts(proposed);
  if (!conflicts.isEmpty()) {
    sheetWarning(this, windowTitle(), conflicts.front().description());
    return;
  }
  m_registry.setShortcuts(proposed);
  QDialog::accept();
}

} // namespace mitcad

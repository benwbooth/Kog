#include <lib/std.mi>
#include "../../cprowidget.mi"
#include <lib/pldir.mi>
#include <lib/fileio.mi>
#include <lib/application.mi>
#include <lib/colormgr.mi>
#include "convert_address.mi"

Function resizeResults(int items);
Function addProvider(list paramname, list paramvalue);
Function String replaceString(string baseString, string toreplace, string replacedby);
Function int searchInListForItem(String input);
Function selectSaved();
Function surfSelected();
Function initLoadFiles();
Function updateDDList();
Function ddlOpenClose();


Global Container results_container;
Global Layout results_layout;

Global Group myGroup;
Global GuiList myList;
Global XmlDoc myDoc;
Global List loaded_P_Names, loaded_P_Url, loaded_P_Icons;
Global int sourceNo, h_tune;
Global boolean onetime, continueLoad;
Global Text ddBoxText;
Global ToggleButton enabledSwitch;
Global Button menuOptions, dropListButton;
Global PopUpMenu popMenu;
Global GuiObject browserXUI, fakeSB;
Global Browser myBrowser;
Global Layer ddlMouseT, ddlIcon;
Global Button bback, bffwd, brefresh, bstop;

Global ColorMgr cm;

Global Timer focus_callback;

System.onScriptLoaded(){
	//setPublicInt("ClassicPro.BrowserPro.loaded", 1);
	initWidget();
	
	results_container = newDynamicContainer("browserpro");
	results_layout = results_container.getLayout("normal");

	myGroup = getScriptGroup();
	myList = results_layout.findObject("BrowserPro.list");
	ddBoxText = myGroup.findObject("browserpro.ddl.text");
	menuOptions = myGroup.findObject("cpro.widget.browserpro.options");
	dropListButton = myGroup.findObject("dropdownlist.button");
	fakeSB = myGroup.findObject("browserpro.ddlist");
	myBrowser = myGroup.findObject("browserpro.browser");
	ddlMouseT = myGroup.findObject("browserpro.ddl.mousetrap");
	ddlIcon = myGroup.findObject("browserpro.ddl.icon");
	bback = myGroup.findObject("browser.back");
	bffwd = myGroup.findObject("browser.fwd");
	brefresh = myGroup.findObject("browser.refresh");
	bstop = myGroup.findObject("browser.stop");

	focus_callback = new Timer;
	focus_callback.setDelay(100);

	cm = new ColorMgr;

	myList.setIconWidth(16);
	myList.setShowIcons(1);
	sourceNo=0;
}

myGroup.onSetVisible(boolean onOff){
	if(onOff){
		initLoadFiles();
		if(continueLoad) surfSelected();
		else myGroup.hide();
	}
}

cm.onColorThemeChanged(String newtheme)
{
	if (onCTChangeReload)
	{
		surfSelected();
	}
}

System.onScriptUnloading (){
	//setPublicInt("ClassicPro.BrowserPro.loaded", 0);
	myGroup.hide();
	focus_callback.stop();
	delete loaded_P_Names;
	delete loaded_P_Url;
	delete focus_callback;
	delete cm;
}


initLoadFiles(){
	if(!onetime){
		loaded_P_Names = new List;
		loaded_P_Url = new List;
		loaded_P_Icons = new List;
		
		myDoc = new XmlDoc;
		String temp = Application.GetApplicationPath()+"\Plugins\ClassicPro\engine\xui\CentroSUI\_v2\Reader\source\_"+strlower(System.getLanguageId())+".xml";
		myDoc.load (temp);
		if(!myDoc.exists()) temp = Application.GetApplicationPath()+"\Plugins\ClassicPro\engine\xui\CentroSUI\_v2\Reader\source\_en-us.xml";
		myDoc.load (temp);
		if(!myDoc.exists()){
			debug("No source file found! Please make sure you installed ClassicPro correct.");
			return;
		}
		
		continueLoad=true;
		

		myDoc.parser_addCallback("WasabiXML/BrowserPro/*");
		myDoc.parser_start();
		myDoc.parser_destroy();
		delete myDoc;

		onetime=true;

		selectSaved();
	}
}

addProvider(list paramname, list paramvalue){
	myList.addItem("Read error");
	for(int i=0; i<paramname.getNumItems(); i++){
		if(strlower(paramname.enumItem(i))=="name"){
			myList.setItemLabel(sourceNo, paramvalue.enumItem(i));
		}
		else if(strlower(paramname.enumItem(i))=="comment"){
			myList.setSubItem(sourceNo, 1, paramvalue.enumItem(i));
		}
		else if(strlower(paramname.enumItem(i))=="url"){
			loaded_P_Url.addItem(paramvalue.enumItem(i));
		}
		else if(strlower(paramname.enumItem(i))=="icon"){
			myList.setItemIcon(sourceNo, paramvalue.enumItem(i));
			loaded_P_Icons.addItem(paramvalue.enumItem(i));
		}		
	}
	loaded_P_Names.addItem(myList.getItemLabel(sourceNo, 0));
	sourceNo++;
}

myDoc.parser_onCallback (String xmlpath, String xmltag, list paramname, list paramvalue){
	if(strlower(xmltag) == "sourceitem"){
		addProvider(paramname, paramvalue);
	}
}

resizeResults(int items){
	if(items>22) items=22;
	if(items>1) h_tune=24;
	else h_tune=19;

	results_layout.setXmlParam("h", integerToString(h_tune+items*17+1));
}

dropListButton.onLeftClick(){
	ddlOpenClose();
}
ddlMouseT.onLeftButtonUp(int x, int y){
	ddlOpenClose();
}

ddlOpenClose(){
	if(results_layout.isVisible()) results_layout.hide();
	else{
		results_layout.setXmlParam("x", integerToString(fakeSB.clientToScreenX(fakeSB.getLeft())));
		results_layout.setXmlParam("y", integerToString(fakeSB.clientToScreenY(fakeSB.getTop() + fakeSB.getHeight())));
		results_layout.setXmlParam("w", integerToString(fakeSB.getWidth()));
	
		results_layout.show();
		resizeResults(loaded_P_Names.getNumItems());
		results_layout.setXmlParam("ontop", "1"); //dunno
		results_layout.setfocus();
		focus_callback.start();
	}
}

focus_callback.onTimer ()
{
	if (!results_layout.isActive())
	{
		focus_callback.stop();
		if (!results_layout) return;
		results_layout.hide();
	}
}

selectSaved(){
	if(!onetime) return;
	int i = searchInListForItem(getPublicString("ClassicPro.BrowserPro", "0"));
	//if(getPublicString("ClassicPro.BrowserPro", "0")) setPublicString("ClassicPro.BrowserPro", myList.getItemLabel(0, 0)); //wtf was this???? forgot :P
	myList.setSelected(i,1);
	updateDDList();
}
searchInListForItem(String input){
	for(int i=0; i<myList.getNumItems();i++){
		if(input == myList.getItemLabel(i, 0)) return i;
	}
	
	setPublicString("ClassicPro.BrowserPro", myList.getItemLabel(0, 0)); //if nothing was found
	return 0;
}
surfSelected(){
	if(!onetime) return;
	String myUrl = loaded_P_Url.enumItem(loaded_P_Names.findItem(getPublicString("ClassicPro.BrowserPro", "0")));
	myBrowser.navigateUrl(prepareCustomUrl(myUrl));
}
System.onTitleChange(String newtitle){
	if(myGroup.isVisible()){
		surfSelected();
	}
}
myList.onLeftClick(Int itemnum){
	setPublicString("ClassicPro.BrowserPro", myList.getItemLabel(itemnum, 0));
	surfSelected();
	results_layout.hide();
	updateDDList();
}

updateDDList(){
	if(!onetime) return;
	ddBoxText.setText(getPublicString("ClassicPro.BrowserPro", "0"));
	String iconsID = loaded_P_Icons.enumItem(loaded_P_Names.findItem(getPublicString("ClassicPro.BrowserPro", "0")));
	ddlIcon.setXmlParam("image", iconsID);
}


//----------------------------------------------------------------------------------------------------------------
// Browser main controls.
//----------------------------------------------------------------------------------------------------------------

bback.onLeftClick(){
	myBrowser.back();
}
bffwd.onLeftClick(){
	myBrowser.forward();
}
brefresh.onLeftClick(){
	surfSelected();
}
bstop.onLeftClick(){
	myBrowser.stop();
}
